use block2::RcBlock;
use objc2::{
    class,
    encode::{Encode, Encoding, RefEncode},
    msg_send, msg_send_id,
    rc::{autoreleasepool, Retained},
    runtime::{AnyObject, Bool},
};
use objc2_foundation::{NSError, NSNumber, NSString, NSURL};
use std::{
    ffi::c_void,
    path::Path,
    ptr,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

#[link(name = "AVFoundation", kind = "framework")]
extern "C" {
    static AVFileTypeMPEG4: *const NSString;
    static AVMediaTypeVideo: *const NSString;
    static AVVideoCodecKey: *const NSString;
    static AVVideoCodecTypeH264: *const NSString;
    static AVVideoWidthKey: *const NSString;
    static AVVideoHeightKey: *const NSString;
    static AVVideoCompressionPropertiesKey: *const NSString;
    static AVVideoAverageBitRateKey: *const NSString;
    static AVVideoProfileLevelKey: *const NSString;
    static AVVideoProfileLevelH264HighAutoLevel: *const NSString;
    static AVVideoMaxKeyFrameIntervalKey: *const NSString;
}

#[link(name = "CoreVideo", kind = "framework")]
extern "C" {
    static kCVPixelBufferPixelFormatTypeKey: *const NSString;
    static kCVPixelBufferWidthKey: *const NSString;
    static kCVPixelBufferHeightKey: *const NSString;

    fn CVPixelBufferCreate(
        allocator: *const c_void,
        width: usize,
        height: usize,
        pixel_format_type: u32,
        pixel_buffer_attributes: *const c_void,
        pixel_buffer_out: *mut *mut CVBuffer,
    ) -> i32;
    fn CVPixelBufferPoolCreatePixelBuffer(
        allocator: *const c_void,
        pixel_buffer_pool: *mut CVPixelBufferPool,
        pixel_buffer_out: *mut *mut CVBuffer,
    ) -> i32;
    fn CVPixelBufferLockBaseAddress(pixel_buffer: *mut CVBuffer, lock_flags: u64) -> i32;
    fn CVPixelBufferUnlockBaseAddress(pixel_buffer: *mut CVBuffer, unlock_flags: u64) -> i32;
    fn CVPixelBufferGetBaseAddress(pixel_buffer: *mut CVBuffer) -> *mut u8;
    fn CVPixelBufferGetBytesPerRow(pixel_buffer: *mut CVBuffer) -> usize;
    fn CVPixelBufferRelease(pixel_buffer: *mut CVBuffer);
}

#[repr(C)]
struct CVBuffer {
    _private: [u8; 0],
}

// SAFETY: Matches CoreVideo's opaque `CVPixelBufferRef` (`^{__CVBuffer=}`).
unsafe impl RefEncode for CVBuffer {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("__CVBuffer", &[]));
}

#[repr(C)]
struct CVPixelBufferPool {
    _private: [u8; 0],
}

// SAFETY: Matches CoreVideo's opaque `CVPixelBufferPoolRef` (`^{__CVPixelBufferPool=}`).
unsafe impl RefEncode for CVPixelBufferPool {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("__CVPixelBufferPool", &[]));
}

const K_CV_PIXEL_FORMAT_32BGRA: u32 = u32::from_be_bytes(*b"BGRA");
const K_CMTIME_FLAGS_VALID: u32 = 1;
const AV_WRITER_STATUS_COMPLETED: isize = 2;
const AV_WRITER_STATUS_FAILED: isize = 3;
const AV_WRITER_STATUS_CANCELLED: isize = 4;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct CMTime {
    value: i64,
    timescale: i32,
    flags: u32,
    epoch: i64,
}

impl CMTime {
    fn from_millis(ms: u64) -> Self {
        Self {
            value: ms.min(i64::MAX as u64) as i64,
            timescale: 1000,
            flags: K_CMTIME_FLAGS_VALID,
            epoch: 0,
        }
    }
}

// SAFETY: Layout matches CoreMedia's CMTime struct `{?=qiIq}` on 64-bit macOS.
unsafe impl Encode for CMTime {
    const ENCODING: Encoding = Encoding::Struct(
        "?",
        &[i64::ENCODING, i32::ENCODING, u32::ENCODING, i64::ENCODING],
    );
}

unsafe impl RefEncode for CMTime {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
}

pub(super) struct NativeMp4Writer {
    writer: Retained<AnyObject>,
    input: Retained<AnyObject>,
    adaptor: Retained<AnyObject>,
    last_pts_ms: Option<u64>,
    last_end_ms: u64,
    last_frame: Option<(Vec<u8>, usize, u32, u32)>,
}

impl NativeMp4Writer {
    pub(super) fn create(
        temp_path: &Path,
        width: u32,
        height: u32,
        fps: u32,
        bitrate: u32,
    ) -> Result<Self, String> {
        let path_str = temp_path
            .to_str()
            .ok_or("MP4 output path is not valid UTF-8")?;
        autoreleasepool(|_| unsafe {
            let url = NSURL::fileURLWithPath(&NSString::from_str(path_str));
            let file_type = AVFileTypeMPEG4
                .as_ref()
                .ok_or("AVFileTypeMPEG4 symbol unavailable")?;
            let mut err: *mut NSError = ptr::null_mut();
            let writer: Option<Retained<AnyObject>> = msg_send_id![
                class!(AVAssetWriter),
                assetWriterWithURL: &*url,
                fileType: file_type,
                error: &mut err
            ];
            let writer = match writer {
                Some(writer) => writer,
                None => return Err(format_ns_error(err, "Could not create AVAssetWriter")),
            };

            let codec_key = AVVideoCodecKey
                .as_ref()
                .ok_or("AVVideoCodecKey unavailable")?;
            let codec_h264 = AVVideoCodecTypeH264
                .as_ref()
                .ok_or("AVVideoCodecTypeH264 unavailable")?;
            let width_key = AVVideoWidthKey
                .as_ref()
                .ok_or("AVVideoWidthKey unavailable")?;
            let height_key = AVVideoHeightKey
                .as_ref()
                .ok_or("AVVideoHeightKey unavailable")?;
            let compression_key = AVVideoCompressionPropertiesKey
                .as_ref()
                .ok_or("AVVideoCompressionPropertiesKey unavailable")?;
            let bitrate_key = AVVideoAverageBitRateKey
                .as_ref()
                .ok_or("AVVideoAverageBitRateKey unavailable")?;
            let profile_key = AVVideoProfileLevelKey
                .as_ref()
                .ok_or("AVVideoProfileLevelKey unavailable")?;
            let profile_high = AVVideoProfileLevelH264HighAutoLevel
                .as_ref()
                .ok_or("AVVideoProfileLevelH264HighAutoLevel unavailable")?;
            let keyframe_key = AVVideoMaxKeyFrameIntervalKey
                .as_ref()
                .ok_or("AVVideoMaxKeyFrameIntervalKey unavailable")?;

            let width_num = NSNumber::numberWithUnsignedInt(width);
            let height_num = NSNumber::numberWithUnsignedInt(height);
            let bitrate_num = NSNumber::numberWithUnsignedInt(bitrate);
            let keyframe_num = NSNumber::numberWithUnsignedInt((fps * 2).max(1));

            let compression = make_dict(&[
                (
                    bitrate_key,
                    &*bitrate_num as *const NSNumber as *const AnyObject,
                ),
                (
                    profile_key,
                    profile_high as *const NSString as *const AnyObject,
                ),
                (
                    keyframe_key,
                    &*keyframe_num as *const NSNumber as *const AnyObject,
                ),
            ]);
            let output_settings = make_dict(&[
                (codec_key, codec_h264 as *const NSString as *const AnyObject),
                (
                    width_key,
                    &*width_num as *const NSNumber as *const AnyObject,
                ),
                (
                    height_key,
                    &*height_num as *const NSNumber as *const AnyObject,
                ),
                (compression_key, &*compression as *const AnyObject),
            ]);

            let media_type = AVMediaTypeVideo
                .as_ref()
                .ok_or("AVMediaTypeVideo unavailable")?;
            let input: Option<Retained<AnyObject>> = msg_send_id![
                class!(AVAssetWriterInput),
                assetWriterInputWithMediaType: media_type,
                outputSettings: &*output_settings
            ];
            let input = input.ok_or("Could not create AVAssetWriterInput")?;
            let _: () = msg_send![&*input, setExpectsMediaDataInRealTime: Bool::NO];

            let can_add: Bool = msg_send![&*writer, canAddInput: &*input];
            if !can_add.as_bool() {
                return Err("AVAssetWriter rejected H.264 video input".into());
            }
            let _: () = msg_send![&*writer, addInput: &*input];

            let fmt_key = kCVPixelBufferPixelFormatTypeKey
                .as_ref()
                .ok_or("kCVPixelBufferPixelFormatTypeKey unavailable")?;
            let pb_width_key = kCVPixelBufferWidthKey
                .as_ref()
                .ok_or("kCVPixelBufferWidthKey unavailable")?;
            let pb_height_key = kCVPixelBufferHeightKey
                .as_ref()
                .ok_or("kCVPixelBufferHeightKey unavailable")?;
            let fmt_num = NSNumber::numberWithUnsignedInt(K_CV_PIXEL_FORMAT_32BGRA);
            let pb_attrs = make_dict(&[
                (fmt_key, &*fmt_num as *const NSNumber as *const AnyObject),
                (
                    pb_width_key,
                    &*width_num as *const NSNumber as *const AnyObject,
                ),
                (
                    pb_height_key,
                    &*height_num as *const NSNumber as *const AnyObject,
                ),
            ]);

            let adaptor: Option<Retained<AnyObject>> = msg_send_id![
                class!(AVAssetWriterInputPixelBufferAdaptor),
                assetWriterInputPixelBufferAdaptorWithAssetWriterInput: &*input,
                sourcePixelBufferAttributes: &*pb_attrs
            ];
            let adaptor = adaptor.ok_or("Could not create pixel buffer adaptor")?;

            let started: Bool = msg_send![&*writer, startWriting];
            if !started.as_bool() {
                return Err(writer_error(
                    &writer,
                    "AVAssetWriter failed to start writing",
                ));
            }
            let _: () = msg_send![&*writer, startSessionAtSourceTime: CMTime::from_millis(0)];

            Ok(Self {
                writer,
                input,
                adaptor,
                last_pts_ms: None,
                last_end_ms: 0,
                last_frame: None,
            })
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
        let pts_ms = match self.last_pts_ms {
            Some(previous) if pts_ms <= previous => previous + 1,
            _ => pts_ms,
        };
        self.append_buffer(bgra, src_stride, width, height, pts_ms)?;
        self.last_pts_ms = Some(pts_ms);
        self.last_end_ms = pts_ms.saturating_add(duration_ms);
        self.last_frame = Some((bgra.to_vec(), src_stride, width, height));
        Ok(())
    }

    fn append_buffer(
        &self,
        bgra: &[u8],
        src_stride: usize,
        width: u32,
        height: u32,
        pts_ms: u64,
    ) -> Result<(), String> {
        autoreleasepool(|_| unsafe {
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                let status: isize = msg_send![&*self.writer, status];
                if status == AV_WRITER_STATUS_FAILED || status == AV_WRITER_STATUS_CANCELLED {
                    return Err(writer_error(
                        &self.writer,
                        "AVAssetWriter failed while writing",
                    ));
                }
                let ready: Bool = msg_send![&*self.input, isReadyForMoreMediaData];
                if ready.as_bool() {
                    break;
                }
                if Instant::now() >= deadline {
                    return Err("Timed out waiting for AVAssetWriter input".into());
                }
                std::thread::sleep(Duration::from_millis(2));
            }

            let mut pb: *mut CVBuffer = ptr::null_mut();
            let pool: *mut CVPixelBufferPool = msg_send![&*self.adaptor, pixelBufferPool];
            let cv_status = if !pool.is_null() {
                CVPixelBufferPoolCreatePixelBuffer(ptr::null(), pool, &mut pb)
            } else {
                CVPixelBufferCreate(
                    ptr::null(),
                    width as usize,
                    height as usize,
                    K_CV_PIXEL_FORMAT_32BGRA,
                    ptr::null(),
                    &mut pb,
                )
            };
            if cv_status != 0 || pb.is_null() {
                return Err(format!(
                    "Could not allocate CVPixelBuffer (code {cv_status})"
                ));
            }

            let copy_result = (|| -> Result<(), String> {
                let lock = CVPixelBufferLockBaseAddress(pb, 0);
                if lock != 0 {
                    return Err(format!("Could not lock CVPixelBuffer (code {lock})"));
                }
                let base = CVPixelBufferGetBaseAddress(pb);
                let dst_stride = CVPixelBufferGetBytesPerRow(pb);
                let row_bytes = width as usize * 4;
                if base.is_null() || dst_stride < row_bytes {
                    CVPixelBufferUnlockBaseAddress(pb, 0);
                    return Err("Invalid CVPixelBuffer layout".into());
                }
                for row in 0..height as usize {
                    let src_offset = row * src_stride;
                    let dst_offset = row * dst_stride;
                    ptr::copy_nonoverlapping(
                        bgra.as_ptr().add(src_offset),
                        base.add(dst_offset),
                        row_bytes,
                    );
                }
                CVPixelBufferUnlockBaseAddress(pb, 0);

                let ok: Bool = msg_send![
                    &*self.adaptor,
                    appendPixelBuffer: pb,
                    withPresentationTime: CMTime::from_millis(pts_ms)
                ];
                if !ok.as_bool() {
                    return Err(writer_error(
                        &self.writer,
                        "AVAssetWriter failed to append video frame",
                    ));
                }
                Ok(())
            })();

            CVPixelBufferRelease(pb);
            copy_result
        })
    }

    pub(super) fn finish(mut self) -> Result<(), String> {
        // Hold the final frame to its recorded end timestamp so single-frame or
        // trailing-static clips keep their true duration in players.
        if let (Some(last_pts), Some((frame, stride, width, height))) =
            (self.last_pts_ms, self.last_frame.take())
        {
            if self.last_end_ms > last_pts {
                self.append_buffer(&frame, stride, width, height, self.last_end_ms)?;
            }
        }
        autoreleasepool(|_| unsafe {
            let _: () = msg_send![&*self.input, markAsFinished];
            let done = Arc::new((Mutex::new(false), Condvar::new()));
            let signal = done.clone();
            let block = RcBlock::new(move || {
                let (lock, cvar) = &*signal;
                if let Ok(mut finished) = lock.lock() {
                    *finished = true;
                    cvar.notify_all();
                }
            });
            let _: () = msg_send![&*self.writer, finishWritingWithCompletionHandler: &*block];

            let (lock, cvar) = &*done;
            let mut finished = lock.lock().unwrap_or_else(|e| e.into_inner());
            let timeout = Duration::from_secs(30);
            let deadline = Instant::now() + timeout;
            while !*finished {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    let _: () = msg_send![&*self.writer, cancelWriting];
                    return Err("Timed out finalizing MP4 file".into());
                }
                let (guard, _) = cvar
                    .wait_timeout(finished, remaining)
                    .unwrap_or_else(|e| e.into_inner());
                finished = guard;
            }

            let status: isize = msg_send![&*self.writer, status];
            if status != AV_WRITER_STATUS_COMPLETED {
                return Err(writer_error(
                    &self.writer,
                    "AVAssetWriter failed to finalize",
                ));
            }
            Ok(())
        })
    }

    pub(super) fn cancel(self) {
        autoreleasepool(|_| unsafe {
            let _: () = msg_send![&*self.writer, cancelWriting];
        });
    }
}

unsafe fn make_dict(entries: &[(&NSString, *const AnyObject)]) -> Retained<AnyObject> {
    let keys: Vec<*const AnyObject> = entries
        .iter()
        .map(|(k, _)| *k as *const NSString as *const AnyObject)
        .collect();
    let values: Vec<*const AnyObject> = entries.iter().map(|(_, v)| *v).collect();
    msg_send_id![
        class!(NSDictionary),
        dictionaryWithObjects: values.as_ptr(),
        forKeys: keys.as_ptr(),
        count: entries.len()
    ]
}

unsafe fn writer_error(writer: &AnyObject, fallback: &str) -> String {
    let error: Option<Retained<NSError>> = msg_send_id![writer, error];
    match error {
        Some(err) => {
            let desc = err.localizedDescription().to_string();
            let reason = err
                .localizedFailureReason()
                .map(|r| format!(" ({r})"))
                .unwrap_or_default();
            format!("{desc}{reason} [code {}]", err.code())
        }
        None => fallback.into(),
    }
}

unsafe fn format_ns_error(error: *mut NSError, fallback: &str) -> String {
    match error.as_ref() {
        Some(err) => err.localizedDescription().to_string(),
        None => fallback.into(),
    }
}
