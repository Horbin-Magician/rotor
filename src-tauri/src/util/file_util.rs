use std::{env, fs};

// #[cfg(target_os = "windows")]
// mod win_imports {
//     pub use crate::util::log_util;
//     pub use std::error::Error;
//     pub use std::ffi::c_void;
//     pub use std::process::Command;
//     pub use std::{io, mem, ptr};
//     pub use windows::core::PCWSTR;
//     pub use windows::Win32::Foundation::{BOOL, HWND};
//     pub use windows::Win32::Graphics::Gdi::{
//         self, DeleteObject, GetBitmapBits, BITMAP, BITMAPINFOHEADER, HBITMAP, HGDIOBJ,
//     };
//     pub use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
//     pub use windows::Win32::UI::Shell::{SHGetFileInfoW, ShellExecuteW, SHFILEINFOW, SHGFI_ICON};
//     pub use windows::Win32::UI::WindowsAndMessaging::{
//         DestroyIcon, GetIconInfo, HICON, ICONINFO, SW_SHOWNORMAL,
//     };
// }
// #[cfg(target_os = "windows")]
// use win_imports::*;

pub fn file_exists(path: &str) -> bool {
    fs::metadata(path).is_ok()
}

pub fn get_app_path() -> std::path::PathBuf {
    if let Ok(exe_path) = env::current_exe() {
        let result = exe_path.parent().unwrap_or(std::path::Path::new("."));
        result.to_path_buf()
    } else {
        std::path::Path::new(".").to_path_buf()
    }
}

pub fn get_tmp_path() -> std::path::PathBuf {
    env::temp_dir()
}

pub fn get_userdata_path() -> Option<std::path::PathBuf> {
    if let Some(home_path) = env::home_dir() {
        return Some(home_path.join(".rotor"));
    }
    None
}

// #[cfg(target_os = "windows")]
// pub fn open_file(file_full_name: String) -> Result<(), Box<dyn Error>> {
//     Command::new("explorer.exe").arg(file_full_name).spawn()?;
//     Ok(())
// }

// #[cfg(target_os = "windows")]
// pub fn open_file_admin(file_full_name: String) {
//     let file_path: Vec<u16> = file_full_name
//         .as_str()
//         .encode_utf16()
//         .chain(std::iter::once(0))
//         .collect();
//     let runas_str: Vec<u16> = "runas".encode_utf16().chain(std::iter::once(0)).collect();
//     unsafe {
//         ShellExecuteW(
//             HWND(std::ptr::null_mut()),
//             PCWSTR(runas_str.as_ptr()),
//             PCWSTR(file_path.as_ptr()),
//             PCWSTR::null(),
//             PCWSTR::null(),
//             SW_SHOWNORMAL,
//         )
//     };
// }

// #[cfg(target_os = "windows")]
// pub fn get_icon(path: &str) -> Option<slint::Image> {
//     #[repr(C)]
//     struct Iconheader {
//         id_reserved: i16,
//         id_type: i16,
//         id_count: i16,
//     }

//     // An array of Icondirs immediately follow the Iconheader
//     #[repr(C)]
//     struct Icondir {
//         b_width: u8,
//         b_height: u8,
//         b_color_count: u8,
//         b_reserved: u8,
//         w_planes: u16,    // for cursors, this field = wXHotSpot
//         w_bit_count: u16, // for cursors, this field = wYHotSpot
//         dw_bytes_in_res: u32,
//         dw_image_offset: u32, // file-offset to the start of ICONIMAGE
//     }

//     fn get_icon_from_file(path: &str) -> HICON {
//         unsafe {
//             let p_path: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
//             let mut file_info = SHFILEINFOW {
//                 dwAttributes: 0,
//                 hIcon: HICON(std::ptr::null_mut()),
//                 iIcon: 0,
//                 szDisplayName: [0_u16; 260],
//                 szTypeName: [0; 80],
//             };
//             let file_info_size = mem::size_of_val(&file_info) as u32;
//             SHGetFileInfoW(
//                 PCWSTR(p_path.as_ptr()),
//                 FILE_ATTRIBUTE_NORMAL,
//                 Some(&mut file_info),
//                 file_info_size,
//                 SHGFI_ICON,
//             );
//             file_info.hIcon
//         }
//     }

//     fn get_bitmap_count(bitmap: &BITMAP) -> i32 {
//         let mut n_width_bytes = bitmap.bmWidthBytes;

//         // bitmap scanlines MUST be a multiple of 4 bytes when stored inside a bitmap resource, so round up if necessary
//         if n_width_bytes & 3 != 0 {
//             n_width_bytes = (n_width_bytes + 4) & !3;
//         }
//         n_width_bytes * bitmap.bmHeight
//     }

//     fn write_icon_data_to_memory(
//         mem: &mut [u8],
//         h_bitmap: HBITMAP,
//         bmp: &BITMAP,
//         bitmap_byte_count: usize,
//     ) {
//         unsafe {
//             let mut icon_data = vec![0; bitmap_byte_count];

//             GetBitmapBits(
//                 h_bitmap,
//                 bitmap_byte_count as i32,
//                 icon_data.as_mut_ptr() as *mut c_void,
//             );

//             // bitmaps are stored inverted (vertically) when on disk so write out each line in turn.
//             // Also, the bitmaps are stored in packed in memory - scanlines are NOT 32bit aligned, just 1-after-the-other
//             let mut pos = 0;
//             for i in (0..bmp.bmHeight).rev() {
//                 // Write the bitmap scanline
//                 ptr::copy_nonoverlapping(
//                     icon_data[(i * bmp.bmWidthBytes) as usize..].as_ptr(),
//                     mem[pos..].as_mut_ptr(),
//                     bmp.bmWidthBytes as usize,
//                 ); // 1 line of BYTES
//                 pos += bmp.bmWidthBytes as usize;
//                 // extend to a 32bit boundary (in the file) if necessary
//                 if bmp.bmWidthBytes & 3 != 0 {
//                     let padding: [u8; 4] = [0; 4];
//                     ptr::copy_nonoverlapping(
//                         padding.as_ptr(),
//                         mem[pos..].as_mut_ptr(),
//                         (4 - bmp.bmWidthBytes % 4) as usize,
//                     );
//                     pos += (4 - bmp.bmWidthBytes % 4) as usize;
//                 } // 试一试案例中的文件是否能识别？
//             }
//         }
//     }

//     if !file_exists(path) {
//         return None;
//     }

//     let h_icon = get_icon_from_file(path);

//     // init and get nesesary message
//     let icon_header = Iconheader {
//         id_count: 1, // number of Icondirs
//         id_reserved: 0,
//         id_type: 1, // Type 1 = ICON (type 2 = CURSOR)
//     };

//     let mut icon_info = ICONINFO {
//         fIcon: BOOL(0),
//         hbmColor: HBITMAP(std::ptr::null_mut()),
//         hbmMask: HBITMAP(std::ptr::null_mut()),
//         xHotspot: 0,
//         yHotspot: 0,
//     };
//     let mut bmp_color = BITMAP {
//         bmBits: ptr::null_mut(),
//         bmBitsPixel: 0,
//         bmHeight: 0,
//         bmPlanes: 0,
//         bmType: 0,
//         bmWidth: 0,
//         bmWidthBytes: 0,
//     };
//     let mut bmp_mask = BITMAP {
//         bmBits: ptr::null_mut(),
//         bmBitsPixel: 0,
//         bmHeight: 0,
//         bmPlanes: 0,
//         bmType: 0,
//         bmWidth: 0,
//         bmWidthBytes: 0,
//     };

//     unsafe {
//         GetIconInfo(h_icon, &mut icon_info)
//             .unwrap_or_else(|e| log_util::log_error(format!("Failed to get icon info: {:?}", e)));
//         Gdi::GetObjectW(
//             icon_info.hbmColor,
//             mem::size_of_val(&bmp_color) as i32,
//             Some(&mut bmp_color as *mut BITMAP as *mut c_void),
//         );
//         Gdi::GetObjectW(
//             icon_info.hbmMask,
//             mem::size_of_val(&bmp_mask) as i32,
//             Some(&mut bmp_mask as *mut BITMAP as *mut c_void),
//         );
//     }

//     let icon_header_size = mem::size_of::<Iconheader>();
//     let icon_dir_size = mem::size_of::<Icondir>();
//     let info_header_size = mem::size_of::<BITMAPINFOHEADER>();
//     let bitmap_bytes_count = get_bitmap_count(&bmp_color) as usize;
//     let mask_bytes_count = get_bitmap_count(&bmp_mask) as usize;
//     let image_bytes_count = bitmap_bytes_count + mask_bytes_count;
//     let complete_size = icon_header_size + icon_dir_size + info_header_size + image_bytes_count;

//     let bi_header = BITMAPINFOHEADER {
//         biSize: info_header_size as u32,
//         biWidth: bmp_color.bmWidth,
//         biHeight: bmp_color.bmHeight * 2, // height of color+mono
//         biPlanes: bmp_color.bmPlanes,
//         biBitCount: bmp_color.bmBitsPixel,
//         biSizeImage: image_bytes_count as u32,
//         biClrImportant: 0,
//         biClrUsed: 0,
//         biCompression: 0,
//         biXPelsPerMeter: 0,
//         biYPelsPerMeter: 0,
//     };

//     let mut bytes = vec![0; complete_size];

//     // 1.write the icon_header
//     unsafe {
//         let byte_ptr: *mut u8 = &icon_header as *const Iconheader as *mut u8;
//         ptr::copy_nonoverlapping(byte_ptr, bytes.as_mut_ptr(), icon_header_size);
//     }

//     // 2.write Icondir
//     let pos = icon_header_size;

//     let color_count = if bmp_color.bmBitsPixel >= 8 {
//         0
//     } else {
//         1 << (bmp_color.bmBitsPixel * bmp_color.bmPlanes)
//     };

//     let icon_dir = Icondir {
//         b_width: bmp_color.bmWidth as u8,
//         b_height: bmp_color.bmHeight as u8,
//         b_color_count: color_count,
//         b_reserved: 0,
//         w_planes: bmp_color.bmPlanes,
//         w_bit_count: bmp_color.bmBitsPixel,
//         dw_bytes_in_res: (mem::size_of::<BITMAPINFOHEADER>() + image_bytes_count) as u32,
//         dw_image_offset: (icon_header_size + 16) as u32,
//     };

//     unsafe {
//         let byte_ptr: *mut u8 = &icon_dir as *const Icondir as *mut u8;
//         ptr::copy_nonoverlapping(byte_ptr, bytes[pos..].as_mut_ptr(), icon_dir_size);
//     }
//     let pos = pos + icon_dir_size;

//     // 3.write bitmap_info_header + colortable
//     unsafe {
//         let byte_ptr: *mut u8 = &bi_header as *const BITMAPINFOHEADER as *mut u8;
//         ptr::copy_nonoverlapping(byte_ptr, bytes[pos..].as_mut_ptr(), info_header_size);
//     }
//     let pos = pos + info_header_size;

//     // 5.write color and mask bitmaps
//     write_icon_data_to_memory(
//         &mut bytes[pos..],
//         icon_info.hbmColor,
//         &bmp_color,
//         bitmap_bytes_count as usize,
//     );
//     let pos = pos + bitmap_bytes_count as usize;
//     write_icon_data_to_memory(
//         &mut bytes[pos..],
//         icon_info.hbmMask,
//         &bmp_mask,
//         mask_bytes_count as usize,
//     );

//     // clear
//     unsafe {
//         DestroyIcon(h_icon)
//             .unwrap_or_else(|e| log_util::log_error(format!("Failed to destroy icon: {:?}", e)));

//         if DeleteObject(HGDIOBJ::from(icon_info.hbmColor)) == false {
//             log_util::log_error("Failed to DeleteObject".to_string());
//         }
//         if DeleteObject(HGDIOBJ::from(icon_info.hbmMask)) == false {
//             log_util::log_error("Failed to DeleteObject".to_string());
//         }
//     }

//     // convert and output
//     let im: image::DynamicImage = image::load_from_memory(&bytes).unwrap_or_default();
//     let pixel_buffer =
//         SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(im.as_bytes(), im.width(), im.height());
//     Some(slint::Image::from_rgba8(pixel_buffer))
// }
