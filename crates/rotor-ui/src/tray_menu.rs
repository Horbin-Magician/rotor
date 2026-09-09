//! Cached GDI resources for the native tray menu; no GPUI window is created.
use windows::Win32::{
    Foundation::{COLORREF, HWND, POINT, RECT, SIZE},
    Graphics::Gdi::*,
    UI::{
        Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW},
        Controls::{DRAWITEMSTRUCT, ODS_SELECTED},
        HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI, SystemParametersInfoForDpi},
        WindowsAndMessaging::*,
    },
};

pub struct NativeMenuPainter {
    labels: [Vec<u16>; 2],
    resources: Option<Resources>,
}

struct Resources {
    dpi: u32,
    dark: bool,
    font: HFONT,
    background: HBRUSH,
    selected: HBRUSH,
    border: HBRUSH,
    border_color: COLORREF,
    foreground: COLORREF,
    selected_foreground: COLORREF,
    width: u32,
    height: u32,
    padding: i32,
}

impl Drop for Resources {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(self.font.into());
            let _ = DeleteObject(self.background.into());
            let _ = DeleteObject(self.selected.into());
            let _ = DeleteObject(self.border.into());
        }
    }
}

impl NativeMenuPainter {
    pub fn new(chinese: bool) -> Self {
        Self {
            labels: if chinese {
                ["设置", "退出"]
            } else {
                ["Settings", "Quit"]
            }
            .map(|label| label.encode_utf16().collect()),
            resources: None,
        }
    }

    /// Refresh only after a DPI, theme or system metrics change. Painting
    /// never creates fonts or brushes. The returned brush is owned here.
    pub fn prepare(&mut self, dark: bool, force: bool) -> Result<HBRUSH, String> {
        let mut cursor = POINT::default();
        // A locked/noninteractive desktop can deny cursor access. Warm the
        // cache at the last DPI (96 initially), then refresh when it is usable.
        let mut dpi = self.resources.as_ref().map_or(96, |r| r.dpi);
        if unsafe { GetCursorPos(&mut cursor) }.is_ok() {
            let monitor = unsafe { MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST) };
            let (mut x, mut y) = (0, 0);
            if unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y) }.is_ok()
                && x > 0
            {
                dpi = x;
            }
        }
        if !force
            && let Some(resources) = &self.resources
            && resources.dpi == dpi
            && resources.dark == dark
        {
            return Ok(resources.background);
        }
        let mut metrics = NONCLIENTMETRICSW {
            cbSize: size_of::<NONCLIENTMETRICSW>() as u32,
            ..Default::default()
        };
        unsafe {
            SystemParametersInfoForDpi(
                SPI_GETNONCLIENTMETRICS.0,
                metrics.cbSize,
                Some((&mut metrics as *mut NONCLIENTMETRICSW).cast()),
                0,
                dpi,
            )
        }
        .map_err(|error| format!("Native menu font metrics: {error}"))?;
        let mut contrast = HIGHCONTRASTW {
            cbSize: size_of::<HIGHCONTRASTW>() as u32,
            ..Default::default()
        };
        unsafe {
            SystemParametersInfoW(
                SPI_GETHIGHCONTRAST,
                contrast.cbSize,
                Some((&mut contrast as *mut HIGHCONTRASTW).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        }
        .map_err(|error| format!("Native menu contrast settings: {error}"))?;
        let (background, foreground, selected, selected_foreground) =
            if contrast.dwFlags.contains(HCF_HIGHCONTRASTON) {
                unsafe {
                    (
                        GetSysColor(COLOR_MENU),
                        GetSysColor(COLOR_MENUTEXT),
                        GetSysColor(COLOR_HIGHLIGHT),
                        GetSysColor(COLOR_HIGHLIGHTTEXT),
                    )
                }
            } else if dark {
                (rgb(0x1c2027), rgb(0xeef2f8), rgb(0x26303f), rgb(0xeef2f8))
            } else {
                (rgb(0xffffff), rgb(0x162033), rgb(0xedf3fa), rgb(0x162033))
            };
        let border_color = COLORREF(if contrast.dwFlags.contains(HCF_HIGHCONTRASTON) {
            unsafe { GetSysColor(COLOR_WINDOWFRAME) }
        } else if dark {
            rgb(0x343a43)
        } else {
            rgb(0xe3e6eb)
        });
        let mut resources = Resources {
            dpi,
            dark,
            font: unsafe { CreateFontIndirectW(&metrics.lfMenuFont) },
            background: unsafe { CreateSolidBrush(COLORREF(background)) },
            selected: unsafe { CreateSolidBrush(COLORREF(selected)) },
            border: unsafe { CreateSolidBrush(border_color) },
            border_color,
            foreground: COLORREF(foreground),
            selected_foreground: COLORREF(selected_foreground),
            width: scaled(80, dpi) as u32,
            height: scaled(24, dpi) as u32,
            padding: scaled(10, dpi),
        };
        if resources.font.is_invalid()
            || resources.background.is_invalid()
            || resources.selected.is_invalid()
            || resources.border.is_invalid()
        {
            return Err("Could not allocate native menu drawing resources".into());
        }
        let dc = unsafe { GetDC(None) };
        if dc.is_invalid() {
            return Err("Could not measure native menu font".into());
        }
        let saved = unsafe { SelectObject(dc, resources.font.into()) };
        let mut measured = true;
        for label in &self.labels {
            let mut size = SIZE::default();
            measured &= unsafe { GetTextExtentPoint32W(dc, label, &mut size) }.as_bool();
            resources.width = resources
                .width
                .max((size.cx + resources.padding * 2) as u32);
            resources.height = resources.height.max((size.cy + scaled(6, dpi)) as u32);
        }
        unsafe {
            SelectObject(dc, saved);
            ReleaseDC(None, dc);
        }
        if !measured {
            return Err("Could not measure native menu labels".into());
        }
        let brush = resources.background;
        self.resources = Some(resources);
        Ok(brush)
    }

    pub fn dimensions(&self) -> (u32, u32) {
        self.resources
            .as_ref()
            .map(|r| (r.width, r.height))
            .unwrap_or((80, 28))
    }

    pub fn border_color(&self) -> COLORREF {
        self.resources
            .as_ref()
            .map_or(COLORREF(rgb(0xe3e6eb)), |r| r.border_color)
    }

    /// Shape the native popup in window coordinates. Windows owns a region
    /// after a successful SetWindowRgn; failed transfers remain ours.
    pub fn round_popup(&self, hwnd: HWND) {
        let Some(resources) = &self.resources else {
            return;
        };
        unsafe {
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect).is_err() {
                return;
            }
            let diameter = scaled(12, resources.dpi);
            let region = CreateRoundRectRgn(
                0,
                0,
                rect.right - rect.left + 1,
                rect.bottom - rect.top + 1,
                diameter,
                diameter,
            );
            if !region.is_invalid() && SetWindowRgn(hwnd, Some(region), true) == 0 {
                let _ = DeleteObject(region.into());
            }
        }
    }

    /// Replace the native nonclient edge using the cached theme brushes.
    pub fn draw_popup_frame(&self, hwnd: HWND, composited: bool) {
        unsafe {
            let dc = GetWindowDC(Some(hwnd));
            if dc.is_invalid() {
                return;
            }
            self.draw_popup_frame_to(hwnd, dc, composited);
            ReleaseDC(Some(hwnd), dc);
        }
    }

    /// WM_PRINT supplies its own DC, which can be an offscreen animation frame.
    pub fn draw_popup_frame_to(&self, hwnd: HWND, dc: HDC, composited: bool) {
        let Some(resources) = &self.resources else {
            return;
        };
        unsafe {
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect).is_err() {
                return;
            }
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            resources.draw_frame(dc, width, height, composited);
        }
    }

    /// The structure and its DC must belong to the current WM_DRAWITEM.
    pub fn draw(&self, index: usize, item: &DRAWITEMSTRUCT) {
        let Some(resources) = &self.resources else {
            return;
        };
        let Some(label) = self.labels.get(index) else {
            return;
        };
        let selected = item.itemState.0 & ODS_SELECTED.0 != 0;
        unsafe {
            let saved = SaveDC(item.hDC);
            if saved == 0 {
                return;
            }
            FillRect(
                item.hDC,
                &item.rcItem,
                if selected {
                    resources.selected
                } else {
                    resources.background
                },
            );
            SelectObject(item.hDC, resources.font.into());
            SetBkMode(item.hDC, TRANSPARENT);
            SetTextColor(
                item.hDC,
                if selected {
                    resources.selected_foreground
                } else {
                    resources.foreground
                },
            );
            let mut size = SIZE::default();
            let _ = GetTextExtentPoint32W(item.hDC, label, &mut size);
            let _ = TextOutW(
                item.hDC,
                item.rcItem.left + (item.rcItem.right - item.rcItem.left - size.cx) / 2,
                item.rcItem.top + (item.rcItem.bottom - item.rcItem.top - size.cy) / 2,
                label,
            );
            let _ = RestoreDC(item.hDC, saved);
        }
    }
}

impl Resources {
    fn draw_frame(&self, dc: HDC, width: i32, height: i32, composited: bool) {
        unsafe {
            let diameter = scaled(12, self.dpi);
            // DWM needs an opaque rectangular surface: it supplies the smooth
            // outer mask and colored border during composition. A GDI outline
            // here would leave another aliased curve inside that smooth mask.
            let region = if composited {
                CreateRectRgn(0, 0, width, height)
            } else {
                CreateRoundRectRgn(0, 0, width + 1, height + 1, diameter, diameter)
            };
            if !region.is_invalid() {
                // Native menu painting can repaint its rectangular edge during
                // WM_PAINT as well as WM_NCPAINT. Restore our entire frame last.
                let inset = scaled(3, self.dpi);
                let _ = FrameRgn(dc, region, self.background, inset, inset);
                if !composited {
                    let _ = FrameRgn(dc, region, self.border, 1, 1);
                }
                let _ = DeleteObject(region.into());
            }
        }
    }
}

fn rgb(value: u32) -> u32 {
    ((value & 0xff) << 16) | (value & 0xff00) | ((value >> 16) & 0xff)
}

fn scaled(value: i32, dpi: u32) -> i32 {
    ((i64::from(value) * i64::from(dpi) + 48) / 96) as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rounded_frame_replaces_bright_edges_without_touching_content() {
        let mut painter = NativeMenuPainter::new(true);
        for dark in [false, true] {
            painter.prepare(dark, true).unwrap();
            let resources = painter.resources.as_mut().unwrap();
            for dpi in [96, 144, 192] {
                resources.dpi = dpi;
                let width = scaled(80, dpi);
                let height = scaled(56, dpi);
                unsafe {
                    let screen = GetDC(None);
                    let dc = CreateCompatibleDC(Some(screen));
                    let bitmap = CreateCompatibleBitmap(screen, width, height);
                    ReleaseDC(None, screen);
                    assert!(!dc.is_invalid() && !bitmap.is_invalid());
                    let previous = SelectObject(dc, bitmap.into());
                    // Simulate the native menu repainting a bright rectangle.
                    let _ = PatBlt(dc, 0, 0, width, height, WHITENESS);
                    resources.draw_frame(dc, width, height, false);
                    let mut border = LOGBRUSH::default();
                    assert_ne!(
                        GetObjectW(
                            resources.border.into(),
                            size_of::<LOGBRUSH>() as i32,
                            Some((&mut border as *mut LOGBRUSH).cast())
                        ),
                        0
                    );
                    for (x, y) in [
                        (width / 2, 0),
                        (0, height / 2),
                        (width - 1, height / 2),
                        (width / 2, height - 1),
                    ] {
                        assert_eq!(
                            GetPixel(dc, x, y),
                            border.lbColor,
                            "edge at {x},{y}, DPI {dpi}"
                        );
                    }
                    let diameter = scaled(12, dpi);
                    let region =
                        CreateRoundRectRgn(0, 0, width + 1, height + 1, diameter, diameter);
                    // Check the curved outline too, where rectangular native
                    // borders previously ended abruptly in the supplied image.
                    for y in 0..diameter / 2 {
                        let x = (0..width / 2)
                            .find(|&x| PtInRegion(region, x, y).as_bool())
                            .unwrap();
                        assert_eq!(GetPixel(dc, x, y), border.lbColor);
                    }
                    assert_eq!(GetPixel(dc, 0, 0), COLORREF(rgb(0xffffff)));
                    assert_eq!(GetPixel(dc, width / 2, height / 2), COLORREF(rgb(0xffffff)));
                    let _ = DeleteObject(region.into());
                    // With DWM rounding, all corners must remain opaque menu
                    // background; no GDI staircase or second outline is drawn.
                    resources.draw_frame(dc, width, height, true);
                    let mut background = LOGBRUSH::default();
                    assert_ne!(
                        GetObjectW(
                            resources.background.into(),
                            size_of::<LOGBRUSH>() as i32,
                            Some((&mut background as *mut LOGBRUSH).cast())
                        ),
                        0
                    );
                    for (x, y) in [
                        (0, 0),
                        (width - 1, 0),
                        (0, height - 1),
                        (width - 1, height - 1),
                        (width / 2, 0),
                    ] {
                        assert_eq!(GetPixel(dc, x, y), background.lbColor);
                    }
                    assert_eq!(GetPixel(dc, width / 2, height / 2), COLORREF(rgb(0xffffff)));
                    SelectObject(dc, previous);
                    let _ = DeleteObject(bitmap.into());
                    let _ = DeleteDC(dc);
                }
            }
        }
    }

    #[test]
    fn drawing_uses_cached_brush_and_restores_the_callers_dc() {
        use windows::Win32::{Foundation::RECT, UI::Controls::ODT_MENU};
        let mut painter = NativeMenuPainter::new(true);
        for dark in [false, true] {
            let brush = painter.prepare(dark, true).unwrap();
            assert_eq!(painter.prepare(dark, false).unwrap(), brush);
            let (width, height) = painter.dimensions();
            assert!(width > 0 && height > 0);
            unsafe {
                let screen = GetDC(None);
                assert!(!screen.is_invalid());
                let dc = CreateCompatibleDC(Some(screen));
                let bitmap = CreateCompatibleBitmap(screen, width as i32, height as i32);
                ReleaseDC(None, screen);
                assert!(!dc.is_invalid() && !bitmap.is_invalid());
                let previous = SelectObject(dc, bitmap.into());
                let original_color = COLORREF(rgb(0xabcdef));
                SetTextColor(dc, original_color);
                let mut item = DRAWITEMSTRUCT {
                    CtlType: ODT_MENU,
                    hDC: dc,
                    rcItem: RECT {
                        left: 0,
                        top: 0,
                        right: width as i32,
                        bottom: height as i32,
                    },
                    ..Default::default()
                };
                for selected in [false, true] {
                    item.itemState = if selected {
                        ODS_SELECTED
                    } else {
                        Default::default()
                    };
                    painter.draw(0, &item);
                    let resources = painter.resources.as_ref().unwrap();
                    let expected = if selected {
                        resources.selected
                    } else {
                        resources.background
                    };
                    let mut brush_info = LOGBRUSH::default();
                    assert_ne!(
                        GetObjectW(
                            expected.into(),
                            size_of::<LOGBRUSH>() as i32,
                            Some((&mut brush_info as *mut LOGBRUSH).cast())
                        ),
                        0
                    );
                    assert_eq!(GetPixel(dc, 1, 1), brush_info.lbColor);
                    assert_eq!(GetTextColor(dc), original_color);
                }
                SelectObject(dc, previous);
                let _ = DeleteObject(bitmap.into());
                let _ = DeleteDC(dc);
            }
        }
    }

    #[test]
    fn native_colors_and_dpi_units_match_windows_formats() {
        assert_eq!(rgb(0x1c2027), 0x27201c);
        assert_eq!(scaled(32, 96), 32);
        assert_eq!(scaled(32, 144), 48);
        assert_eq!(scaled(32, 192), 64);
    }
}
