// Transfers screenshot RGBA bytes to the requesting webview through the
// WebView2 SharedBuffer API (Windows only), bypassing JSON IPC serialization.

#[cfg(target_os = "windows")]
mod imp {
    use std::sync::mpsc;

    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Environment12, ICoreWebView2_17, COREWEBVIEW2_SHARED_BUFFER_ACCESS_READ_ONLY,
    };
    use windows_core::{Interface, PCWSTR};

    use rotor_runtime::ScreenshotImage;

    pub async fn get_screenshot_data_shared(
        window: tauri::WebviewWindow,
        request_id: String,
    ) -> Result<(), String> {
        let image = rotor_runtime::resolve_screenshot_image(window.label()).await?;
        let (width, height) = image.dimensions();
        let length = image.bytes().len();

        let meta = serde_json::json!({
            "requestId": request_id,
            "width": width,
            "height": height,
            "length": length,
        })
        .to_string();
        let wide: Vec<u16> = meta.encode_utf16().chain(std::iter::once(0)).collect();

        let (tx, rx) = mpsc::channel::<Result<(), String>>();

        window
            .with_webview(move |webview| {
                let result = post_shared_buffer(&webview, &image, &wide);
                let _ = tx.send(result);
            })
            .map_err(|e| format!("shared-buffer-dispatch: {e}"))?;

        match tokio::task::spawn_blocking(move || rx.recv()).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("shared-buffer-dispatch: webview task dropped".into()),
            Err(e) => Err(format!("shared-buffer-dispatch: {e}")),
        }
        .inspect(|_| log::debug!("shared buffer posted: {length} bytes ({width}x{height})"))
        .inspect_err(|e| log::error!("shared buffer post failed: {e}"))
    }

    fn post_shared_buffer(
        webview: &tauri::webview::PlatformWebview,
        image: &ScreenshotImage,
        wide: &[u16],
    ) -> Result<(), String> {
        let src = image.bytes();

        unsafe {
            let controller = webview.controller();
            let core = controller
                .CoreWebView2()
                .map_err(|e| format!("shared-buffer-unavailable: {e}"))?;
            let wv17 = core
                .cast::<ICoreWebView2_17>()
                .map_err(|e| format!("shared-buffer-unavailable: {e}"))?;
            let env12 = webview
                .environment()
                .cast::<ICoreWebView2Environment12>()
                .map_err(|e| format!("shared-buffer-unavailable: {e}"))?;
            let buffer = env12
                .CreateSharedBuffer(src.len() as u64)
                .map_err(|e| format!("shared-buffer-create: {e}"))?;
            let mut dst: *mut u8 = std::ptr::null_mut();
            buffer
                .Buffer(&mut dst)
                .map_err(|e| format!("shared-buffer-map: {e}"))?;
            std::ptr::copy_nonoverlapping(src.as_ptr(), dst, src.len());
            wv17.PostSharedBufferToScript(
                &buffer,
                COREWEBVIEW2_SHARED_BUFFER_ACCESS_READ_ONLY,
                PCWSTR::from_raw(wide.as_ptr()),
            )
            .map_err(|e| format!("shared-buffer-post: {e}"))?;
        }

        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    pub async fn get_screenshot_data_shared(
        _window: tauri::WebviewWindow,
        _request_id: String,
    ) -> Result<(), String> {
        Err("shared-buffer-unsupported".into())
    }
}

#[tauri::command]
pub async fn get_screenshot_data_shared(
    window: tauri::WebviewWindow,
    request_id: String,
) -> Result<(), String> {
    imp::get_screenshot_data_shared(window, request_id).await
}
