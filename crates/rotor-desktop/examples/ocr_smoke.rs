//! Headless OCR integration check using generated text, never the desktop.
use rotor_canvas::{Annotation, Color, Document, ImagePoint, ImageRect, ImageSize, Renderer};
use rotor_common::{ConfigService, ResourceLocator};
use rotor_runtime::{RuntimeEvent, ServiceOptions, Services};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let arguments = std::env::args().collect::<Vec<_>>();
    let resource_root = match arguments
        .iter()
        .position(|argument| argument == "--resource-dir")
    {
        Some(index) => PathBuf::from(
            arguments
                .get(index + 1)
                .ok_or("Missing --resource-dir value")?,
        ),
        None => root.join("assets"),
    };
    let resources = ResourceLocator::from_root(&resource_root)?;
    resources.verify_native_resources()?;
    let profile = root.join("target/ocr-smoke");
    std::fs::create_dir_all(&profile)?;
    let source = image::RgbaImage::from_pixel(900, 220, image::Rgba([255, 255, 255, 255]));
    let size = ImageSize {
        width: 900,
        height: 220,
    };
    let mut document = Document::new(
        size,
        ImageRect {
            x: 0,
            y: 0,
            width: 900,
            height: 220,
        },
    )?;
    document.add(Annotation::Text {
        origin: ImagePoint { x: 24., y: 24. },
        text: "中文测试 Hello 123".into(),
        font_size: 48.,
        color: Color([0, 0, 0, 255]),
    })?;
    document.add(Annotation::Text {
        origin: ImagePoint { x: 24., y: 110. },
        text: "Rotor OCR 验证".into(),
        font_size: 44.,
        color: Color::RED,
    })?;
    let renderer = Renderer::with_font(
        std::fs::read(resources.resolve(std::path::Path::new("fonts/NotoSansCJKsc-Regular.otf"))?)?,
        false,
    )?;
    let image = renderer.render(&source, document.scene(), size)?;
    image.save(profile.join("fixture.png"))?;
    let config = ConfigService::load_from(&profile)?;
    let (services, events) = Services::new(
        Arc::new(Mutex::new(config)),
        Some(resources),
        ServiceOptions { index_files: false },
    )?;
    let request = services.recognize_text(7, 42, Arc::new(image))?;
    let (sender, receiver) = mpsc::channel();
    let bridge = std::thread::spawn(move || {
        while let Ok(event) = events.recv_blocking() {
            if let RuntimeEvent::OcrFinished {
                id,
                pin_id,
                revision,
                result,
            } = event
            {
                let _ = sender.send((id, pin_id, revision, result));
                break;
            }
        }
    });
    let result = receiver.recv_timeout(Duration::from_secs(60));
    services.shutdown();
    let (id, pin_id, revision, result) = result?;
    bridge.join().map_err(|_| "OCR result bridge failed")?;
    assert_eq!(id, request);
    assert_eq!(pin_id, 7);
    assert_eq!(revision, 42);
    let results = result?;
    let text = results
        .iter()
        .map(|result| result.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    println!("{text}");
    assert!(
        text.contains("中文") && text.contains("123"),
        "generated OCR fixture was not recognized"
    );
    println!("OCR smoke passed: {} merged regions", results.len());
    if std::env::args().any(|argument| argument == "--wait-idle") {
        assert_eq!(rotor_runtime::ocr_cache_loaded(), Some(true));
        println!("Waiting for the existing OCR idle reaper...");
        std::thread::sleep(Duration::from_secs(32));
        assert_eq!(rotor_runtime::ocr_cache_loaded(), Some(false));
        println!("OCR idle release passed");
    }
    Ok(())
}
