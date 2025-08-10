use crate::core::application::Application;
use crate::module::searcher::Searcher;
use std::path::Path;

#[tauri::command]
pub async fn searcher_find(query: String) {
    let mut app = Application::global().lock().unwrap();
    app.get_module("searcher")
        .and_then(|s| s.as_any().downcast_ref::<Searcher>())
        .map(|searcher| searcher.find(query));
}

#[tauri::command]
pub async fn searcher_release() {
    let mut app = Application::global().lock().unwrap();
    app.get_module("searcher")
        .and_then(|s| s.as_any().downcast_ref::<Searcher>())
        .map(|searcher| searcher.release());
}

#[tauri::command]
pub fn open_file(file_path: String) -> Result<(), String> {
    let path = Path::new(&file_path);
    if !path.exists() {
        return Err(format!("File does not exist: {}", file_path));
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &file_path])
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }
    
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&file_path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }
    
    Ok(())
}

#[tauri::command]
pub fn open_file_as_admin(file_path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let path = Path::new(&file_path);
        if !path.exists() {
            return Err(format!("File does not exist: {}", file_path));
        }

        std::process::Command::new("powershell")
            .args([
                "-Command",
                &format!("Start-Process -FilePath '{}' -Verb RunAs", file_path)
            ])
            .spawn()
            .map_err(|e| format!("Failed to open file as admin: {}", e))?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        open_file(file_path)?;
        return Err(format!("MacOS does not support, use normal open instead"));
    }
}
