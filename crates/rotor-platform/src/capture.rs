//! Thread-owned Windows capture resources. No UI handles or entities are retained.

#[cfg(target_os = "windows")]
mod windows_capture {
    use std::{ffi::c_void, ptr};
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GdiFlush, GetDC,
        ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP,
        HDC, HGDIOBJ, SRCCOPY,
    };

    /// Created, used and dropped on a single capture worker thread.
    pub struct DesktopCapture {
        dc: HDC,
        bitmap: HBITMAP,
        previous: HGDIOBJ,
        pixels: *mut c_void,
        dimensions: (u32, u32),
    }

    impl Default for DesktopCapture {
        fn default() -> Self {
            Self {
                dc: HDC::default(),
                bitmap: HBITMAP::default(),
                previous: HGDIOBJ::default(),
                pixels: ptr::null_mut(),
                dimensions: (0, 0),
            }
        }
    }

    impl DesktopCapture {
        fn reset(&mut self) {
            unsafe {
                if !self.dc.is_invalid() {
                    if !self.previous.is_invalid() {
                        SelectObject(self.dc, self.previous);
                    }
                    if !self.bitmap.is_invalid() {
                        let _ = DeleteObject(self.bitmap.into());
                    }
                    let _ = DeleteDC(self.dc);
                }
            }
            self.dc = HDC::default();
            self.bitmap = HBITMAP::default();
            self.previous = HGDIOBJ::default();
            self.pixels = ptr::null_mut();
            self.dimensions = (0, 0);
        }

        /// Returns top-down, opaque BGRA bytes. The returned allocation is owned
        /// by the caller; subsequent captures cannot change a previous frame.
        pub fn capture(
            &mut self,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> Result<Vec<u8>, String> {
            // Acquire the desktop DC for each request, so session/display changes
            // cannot leave a cached desktop DC attached to the wrong desktop.
            let screen = unsafe { GetDC(None) };
            if screen.is_invalid() {
                return Err("Could not acquire desktop DC".into());
            }
            let result = self.copy_from_dc(screen, x, y, width, height);
            unsafe {
                ReleaseDC(None, screen);
            }
            result
        }

        /// Allocate reusable capture resources without reading desktop pixels.
        pub fn prepare(&mut self, width: u32, height: u32) -> Result<(), String> {
            let screen = unsafe { GetDC(None) };
            if screen.is_invalid() {
                return Err("Could not acquire desktop DC".into());
            }
            let result = self.prepare_dc(screen, width, height).map(|_| ());
            unsafe {
                ReleaseDC(None, screen);
            }
            result
        }

        fn prepare_dc(&mut self, screen: HDC, width: u32, height: u32) -> Result<usize, String> {
            let length = (width as usize)
                .checked_mul(height as usize)
                .and_then(|pixels| pixels.checked_mul(4))
                .filter(|length| *length > 0 && *length <= isize::MAX as usize)
                .ok_or("Invalid capture dimensions")?;
            let w = i32::try_from(width).map_err(|_| "Capture width is too large")?;
            let h = i32::try_from(height).map_err(|_| "Capture height is too large")?;
            let result = (|| unsafe {
                if self.dimensions != (width, height) {
                    self.reset();
                    self.dc = CreateCompatibleDC(Some(screen));
                    if self.dc.is_invalid() {
                        return Err("Could not create capture DC".into());
                    }
                    let info = BITMAPINFO {
                        bmiHeader: BITMAPINFOHEADER {
                            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                            biWidth: w,
                            biHeight: -h,
                            biPlanes: 1,
                            biBitCount: 32,
                            biCompression: BI_RGB.0,
                            ..Default::default()
                        },
                        ..Default::default()
                    };
                    self.bitmap = CreateDIBSection(
                        Some(screen),
                        &info,
                        DIB_RGB_COLORS,
                        &mut self.pixels,
                        None,
                        0,
                    )
                    .map_err(|error| error.to_string())?;
                    self.previous = SelectObject(self.dc, self.bitmap.into());
                    if self.previous.is_invalid() || self.pixels.is_null() {
                        return Err("Could not select capture bitmap".into());
                    }
                    self.dimensions = (width, height);
                }
                Ok(length)
            })();
            if result.is_err() {
                self.reset();
            }
            result
        }

        // `source` is a live DC borrowed by the caller on this thread.
        fn copy_from_dc(
            &mut self,
            screen: HDC,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> Result<Vec<u8>, String> {
            let length = self.prepare_dc(screen, width, height)?;
            let (w, h) = (width as i32, height as i32);
            let result = (|| unsafe {
                BitBlt(self.dc, 0, 0, w, h, Some(screen), x, y, SRCCOPY)
                    .map_err(|error| error.to_string())?;
                // DIB memory must not be read before this thread's GDI batch completes.
                GdiFlush().ok().map_err(|error| error.to_string())?;
                let mut bytes =
                    std::slice::from_raw_parts(self.pixels.cast::<u8>(), length).to_vec();
                // A desktop bitmap has no meaningful alpha channel.
                for pixel in bytes.chunks_exact_mut(4) {
                    pixel[3] = 255;
                }
                Ok(bytes)
            })();
            if result.is_err() {
                self.reset();
            }
            result
        }
    }

    impl Drop for DesktopCapture {
        fn drop(&mut self) {
            self.reset();
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn synthetic_dib_capture_is_top_down_opaque_owned_and_reuses_resources() {
            // The source is an in-memory DIB, never the user's desktop.
            let mut source = DesktopCapture::default();
            unsafe {
                source.dc = CreateCompatibleDC(None);
                assert!(!source.dc.is_invalid());
                let info = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: 2,
                        biHeight: -2,
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                source.bitmap =
                    CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut source.pixels, None, 0)
                        .unwrap();
                source.previous = SelectObject(source.dc, source.bitmap.into());
                assert!(!source.previous.is_invalid());
                let pixels = std::slice::from_raw_parts_mut(source.pixels.cast::<u8>(), 16);
                pixels.copy_from_slice(&[
                    10, 20, 30, 0, 40, 50, 60, 0, 70, 80, 90, 0, 100, 110, 120, 0,
                ]);
                let mut capture = DesktopCapture::default();
                let first = capture.copy_from_dc(source.dc, 0, 0, 2, 2).unwrap();
                assert_eq!(
                    first,
                    [10, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255, 100, 110, 120, 255]
                );
                let (dc, bitmap) = (capture.dc, capture.bitmap);
                GdiFlush().ok().unwrap();
                pixels[0] = 99;
                let second = capture.copy_from_dc(source.dc, 0, 0, 2, 2).unwrap();
                assert_eq!(second[0], 99);
                assert_eq!(first[0], 10);
                assert_eq!(capture.dc, dc);
                assert_eq!(capture.bitmap, bitmap);
                assert_eq!(
                    capture.copy_from_dc(source.dc, 1, 1, 1, 1).unwrap(),
                    [100, 110, 120, 255]
                );
                assert!(capture.copy_from_dc(source.dc, 0, 0, 0, 2).is_err());
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub use windows_capture::DesktopCapture;
