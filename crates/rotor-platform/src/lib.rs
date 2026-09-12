//! Native operating-system services for Windows and macOS.

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
compile_error!("Rotor supports only Windows and macOS");

pub mod file_util;
pub mod sys_util;

pub mod capture;
pub mod clipboard;
pub mod cursor;
pub mod desktop;
#[cfg(target_os = "windows")]
pub mod installer;
pub mod monitor;
pub mod overlay;
pub mod selection;
pub mod single_instance;
pub mod startup;
