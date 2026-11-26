use std::{os::windows::ffi::OsStrExt, path::Path};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::RPC_E_CHANGED_MODE,
        Media::MediaFoundation::{
            IMFAttributes, IMFMediaType, IMFSinkWriter, MFCreateAttributes, MFCreateMediaType,
            MFCreateMemoryBuffer, MFCreateSample, MFCreateSinkWriterFromURL, MFMediaType_Video,
            MFShutdown, MFStartup, MFVideoFormat_H264, MFVideoFormat_RGB32,
            MFVideoInterlace_Progressive, MFSTARTUP_NOSOCKET, MF_MT_AVG_BITRATE,
            MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE, MF_MT_INTERLACE_MODE,
            MF_MT_MAJOR_TYPE, MF_MT_PIXEL_ASPECT_RATIO, MF_MT_SUBTYPE,
            MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, MF_SINK_WRITER_DISABLE_THROTTLING, MF_VERSION,
        },
        System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED},
    },
};

struct ComGuard {
    uninitialize: bool,
}

impl ComGuard {
    fn initialize() -> Result<Self, String> {
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr.is_ok() {
            Ok(Self { uninitialize: true })
        } else if hr == RPC_E_CHANGED_MODE {
            Ok(Self {
                uninitialize: false,
            })
        } else {
            Err(windows::core::Error::from(hr).to_string())
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.uninitialize {
            unsafe { CoUninitialize() };
        }
    }
}

struct MfGuard;

impl MfGuard {
    fn startup() -> Result<Self, String> {
        unsafe { MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET) }.map_err(|e| e.to_string())?;
        Ok(Self)
    }
}

impl Drop for MfGuard {
    fn drop(&mut self) {
        let _ = unsafe { MFShutdown() };
    }
}

pub(super) struct NativeMp4Writer {
    // Field drop order is important: release COM objects before MFShutdown / CoUninitialize.
    sink_writer: IMFSinkWriter,
    stream_index: u32,
    last_pts_100ns: Option<i64>,
    _mf: MfGuard,
    _com: ComGuard,
}

impl NativeMp4Writer {
    pub(super) fn create(
        temp_path: &Path,
        width: u32,
        height: u32,
        fps: u32,
        bitrate: u32,
    ) -> Result<Self, String> {
        let com = ComGuard::initialize()?;
        let mf = MfGuard::startup()?;

        let wide: Vec<u16> = dunce::simplified(temp_path)
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();

        let (sink_writer, stream_index) = unsafe {
            let mut attrs: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attrs, 2).map_err(|e| e.to_string())?;
            let attrs = attrs.ok_or("Could not allocate Media Foundation attributes")?;
            attrs
                .SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)
                .map_err(|e| e.to_string())?;
            attrs
                .SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1)
                .map_err(|e| e.to_string())?;

            let sink_writer = MFCreateSinkWriterFromURL(PCWSTR(wide.as_ptr()), None, &attrs)
                .map_err(|e| format!("Could not create MP4 sink writer: {e}"))?;

            let frame_size = (u64::from(width) << 32) | u64::from(height);
            let frame_rate = (u64::from(fps) << 32) | 1u64;
            let aspect_ratio = (1u64 << 32) | 1u64;

            let output_type: IMFMediaType = MFCreateMediaType().map_err(|e| e.to_string())?;
            output_type
                .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
                .map_err(|e| e.to_string())?;
            output_type
                .SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)
                .map_err(|e| e.to_string())?;
            output_type
                .SetUINT32(&MF_MT_AVG_BITRATE, bitrate)
                .map_err(|e| e.to_string())?;
            output_type
                .SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
                .map_err(|e| e.to_string())?;
            output_type
                .SetUINT64(&MF_MT_FRAME_SIZE, frame_size)
                .map_err(|e| e.to_string())?;
            output_type
                .SetUINT64(&MF_MT_FRAME_RATE, frame_rate)
                .map_err(|e| e.to_string())?;
            output_type
                .SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, aspect_ratio)
                .map_err(|e| e.to_string())?;

            let stream_index = sink_writer
                .AddStream(&output_type)
                .map_err(|e| format!("Could not add H.264 stream: {e}"))?;

            let input_type: IMFMediaType = MFCreateMediaType().map_err(|e| e.to_string())?;
            input_type
                .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
                .map_err(|e| e.to_string())?;
            input_type
                .SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)
                .map_err(|e| e.to_string())?;
            input_type
                .SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
                .map_err(|e| e.to_string())?;
            input_type
                .SetUINT64(&MF_MT_FRAME_SIZE, frame_size)
                .map_err(|e| e.to_string())?;
            input_type
                .SetUINT64(&MF_MT_FRAME_RATE, frame_rate)
                .map_err(|e| e.to_string())?;
            input_type
                .SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, aspect_ratio)
                .map_err(|e| e.to_string())?;
            // Positive stride tells Media Foundation the BGRA buffer is top-down.
            input_type
                .SetUINT32(&MF_MT_DEFAULT_STRIDE, width * 4)
                .map_err(|e| e.to_string())?;

            sink_writer
                .SetInputMediaType(stream_index, &input_type, None)
                .map_err(|e| format!("Could not configure video input format: {e}"))?;
            sink_writer
                .BeginWriting()
                .map_err(|e| format!("Could not start writing MP4: {e}"))?;

            (sink_writer, stream_index)
        };

        Ok(Self {
            sink_writer,
            stream_index,
            last_pts_100ns: None,
            _mf: mf,
            _com: com,
        })
    }

    pub(super) fn write_frame(
        &mut self,
        bgra: &[u8],
        src_stride: usize,
        width: u32,
        height: u32,
        pts_ms: u64,
        duration_ms: u64,
    ) -> Result<(), String> {
        let mut pts_100ns = (pts_ms.min((i64::MAX / 10_000) as u64) as i64) * 10_000;
        if let Some(previous) = self.last_pts_100ns {
            if pts_100ns <= previous {
                pts_100ns = previous + 10_000;
            }
        }
        let duration_100ns = (duration_ms.max(1).min((i64::MAX / 10_000) as u64) as i64) * 10_000;
        let row_bytes = width as usize * 4;
        let buffer_len = u32::try_from(row_bytes * height as usize)
            .map_err(|_| "Video frame exceeds Media Foundation buffer limit")?;

        unsafe {
            let buffer = MFCreateMemoryBuffer(buffer_len).map_err(|e| e.to_string())?;
            let mut dst: *mut u8 = std::ptr::null_mut();
            buffer
                .Lock(&mut dst, None, None)
                .map_err(|e| e.to_string())?;
            for row in 0..height as usize {
                std::ptr::copy_nonoverlapping(
                    bgra.as_ptr().add(row * src_stride),
                    dst.add(row * row_bytes),
                    row_bytes,
                );
            }
            buffer.Unlock().map_err(|e| e.to_string())?;
            buffer
                .SetCurrentLength(buffer_len)
                .map_err(|e| e.to_string())?;

            let sample = MFCreateSample().map_err(|e| e.to_string())?;
            sample.AddBuffer(&buffer).map_err(|e| e.to_string())?;
            sample.SetSampleTime(pts_100ns).map_err(|e| e.to_string())?;
            sample
                .SetSampleDuration(duration_100ns)
                .map_err(|e| e.to_string())?;

            self.sink_writer
                .WriteSample(self.stream_index, &sample)
                .map_err(|e| format!("Could not write video frame: {e}"))?;
        }
        self.last_pts_100ns = Some(pts_100ns);
        Ok(())
    }

    pub(super) fn finish(self) -> Result<(), String> {
        unsafe { self.sink_writer.Finalize() }
            .map_err(|e| format!("Could not finalize MP4 file: {e}"))
    }

    pub(super) fn cancel(self) {}
}
