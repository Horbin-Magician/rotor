use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
};

fn create(config: ConfigService) -> (Services, Receiver<RuntimeEvent>) {
    Services::new(
        Arc::new(Mutex::new(config)),
        None,
        ServiceOptions { index_files: false },
    )
    .unwrap()
}

#[test]
fn hidden_pin_restore_is_explicit_preserves_ids_and_does_not_rewrite_records() {
    let directory = tempfile::tempdir().unwrap();
    let (services, events) = create(ConfigService::load_from(directory.path()).unwrap());
    let mut config = pin_config();
    config.minimized = true;
    let created_id = services
        .create_pin(Arc::new(RgbaImage::new(2, 3)), config)
        .unwrap();
    let receive = || {
        services.runtime().block_on(async {
            tokio::time::timeout(Duration::from_secs(3), events.recv())
                .await
                .unwrap()
                .unwrap()
        })
    };
    let RuntimeEvent::Pin(crate::PinEvent::Created {
        id,
        result: Ok(pin),
    }) = receive()
    else {
        panic!("expected created pin");
    };
    assert_eq!(id, created_id);
    let record_path = directory.path().join("pins/record.toml");
    let record = std::fs::read(&record_path).unwrap();
    for (request, expected_reveal, expected_count) in [
        (services.restore_pins().unwrap(), false, 0),
        (services.restore_hidden_pins(Vec::new()).unwrap(), true, 1),
        (services.restore_hidden_pins(vec![pin.id]).unwrap(), true, 0),
    ] {
        let RuntimeEvent::Pin(crate::PinEvent::Restored {
            id,
            reveal,
            result: Ok(restored),
        }) = receive()
        else {
            panic!("expected restored pins");
        };
        assert_eq!(id, request);
        assert_eq!(reveal, expected_reveal);
        assert_eq!(restored.pins.len(), expected_count);
        assert!(restored.warnings.is_empty());
        assert_eq!(std::fs::read(&record_path).unwrap(), record);
    }
}

#[test]
fn shutdown_drains_accepted_configuration_writes_in_order() {
    let directory = tempfile::tempdir().unwrap();
    let (services, _events) = create(ConfigService::load_from(directory.path()).unwrap());
    services
        .save_settings(vec![("theme".into(), "1".into())])
        .unwrap();
    services
        .save_settings(vec![
            ("theme".into(), "2".into()),
            ("unknown".into(), "kept".into()),
        ])
        .unwrap();
    services.shutdown();
    assert!(services.save_settings(vec![]).is_err());
    drop(services);
    let config = ConfigService::load_from(directory.path()).unwrap();
    assert_eq!(config.get_user("theme").map(String::as_str), Some("2"));
    assert_eq!(config.get_user("unknown").map(String::as_str), Some("kept"));
}

fn pin_config() -> crate::ShotterConfig {
    crate::ShotterConfig {
        annotations: Vec::new(),
        monitor_pos: (0, 0),
        monitor_size: (1920, 1080),
        rect: (0, 0, 2, 3),
        image_rect: (0, 0, 2, 3),
        offset: (0, 0),
        zoom_factor: 100,
        mask_label: "ssmask-1".into(),
        minimized: false,
    }
}

#[test]
fn shutdown_drains_accepted_pin_creation_without_an_event_consumer() {
    let directory = tempfile::tempdir().unwrap();
    let (services, _events) = create(ConfigService::load_from(directory.path()).unwrap());
    services
        .create_pin(Arc::new(RgbaImage::new(2, 3)), pin_config())
        .unwrap();
    services.shutdown();
    assert!(services.restore_pins().is_err());
    drop(services);
    let store = rotor_screenshot::pin_store::PinStore::load_from(directory.path()).unwrap();
    let (pins, warnings) = store.load_pins();
    assert!(warnings.is_empty());
    assert_eq!(pins.len(), 1);
}

#[test]
fn pin_update_and_delete_follow_submission_order() {
    let directory = tempfile::tempdir().unwrap();
    let (services, events) = create(ConfigService::load_from(directory.path()).unwrap());
    let request = services
        .create_pin(Arc::new(RgbaImage::new(2, 3)), pin_config())
        .unwrap();
    let created = services.runtime().block_on(async {
        tokio::time::timeout(Duration::from_secs(3), events.recv())
            .await
            .unwrap()
            .unwrap()
    });
    let RuntimeEvent::Pin(crate::PinEvent::Created { id, result }) = created else {
        panic!("expected created pin");
    };
    assert_eq!(id, request);
    let pin = result.unwrap();
    let mut config = pin.config;
    config.offset = (5, -8);
    let updated = services.update_pin(pin.id, config).unwrap();
    let deleted = services.delete_pin(pin.id).unwrap();
    services.runtime().block_on(services.flush_pins()).unwrap();
    assert!(
        matches!(events.try_recv().unwrap(), RuntimeEvent::Pin(crate::PinEvent::Updated { id, result: Ok(_), .. }) if id == updated)
    );
    assert!(
        matches!(events.try_recv().unwrap(), RuntimeEvent::Pin(crate::PinEvent::Deleted { id, result: Ok(()), .. }) if id == deleted)
    );
    assert!(
        rotor_screenshot::pin_store::PinStore::load_from(directory.path())
            .unwrap()
            .load_pins()
            .0
            .is_empty()
    );
}

#[test]
fn pin_export_failure_keeps_record_and_success_removes_it() {
    let directory = tempfile::tempdir().unwrap();
    let (services, events) = create(ConfigService::load_from(directory.path()).unwrap());
    let image = Arc::new(RgbaImage::from_pixel(2, 3, image::Rgba([7, 8, 9, 128])));
    services.create_pin(image.clone(), pin_config()).unwrap();
    let receive = || {
        services.runtime().block_on(async {
            tokio::time::timeout(Duration::from_secs(3), events.recv())
                .await
                .unwrap()
                .unwrap()
        })
    };
    let RuntimeEvent::Pin(crate::PinEvent::Created {
        result: Ok(pin), ..
    }) = receive()
    else {
        panic!("expected created pin");
    };
    let invalid = directory.path().join("directory.png");
    std::fs::create_dir(&invalid).unwrap();
    let failed = services
        .export_pin(
            Some(pin.id),
            image.clone(),
            pin_config(),
            crate::PinExportTarget::File(invalid),
        )
        .unwrap();
    assert!(
        matches!(receive(), RuntimeEvent::Pin(crate::PinEvent::Exported { id, result: Err(_) }) if id == failed)
    );
    assert_eq!(
        rotor_screenshot::pin_store::PinStore::load_from(directory.path())
            .unwrap()
            .load_pins()
            .0
            .len(),
        1
    );
    let output = directory.path().join("export.png");
    let saved = services
        .export_pin(
            Some(pin.id),
            image.clone(),
            pin_config(),
            crate::PinExportTarget::File(output.clone()),
        )
        .unwrap();
    assert!(
        matches!(receive(), RuntimeEvent::Pin(crate::PinEvent::Exported { id, result: Ok(()) }) if id == saved)
    );
    assert_eq!(image::open(output).unwrap().into_rgba8(), *image);
    assert!(
        rotor_screenshot::pin_store::PinStore::load_from(directory.path())
            .unwrap()
            .load_pins()
            .0
            .is_empty()
    );
}

#[test]
fn confirmed_capture_is_cropped_and_persisted_before_shutdown() {
    let directory = tempfile::tempdir().unwrap();
    let (services, _events) = create(ConfigService::load_from(directory.path()).unwrap());
    let image = Arc::new(RgbaImage::from_fn(4, 4, |x, y| {
        image::Rgba([x as u8, y as u8, 42, 255])
    }));
    let mut config = pin_config();
    config.monitor_size = (4, 4);
    config.rect = (1, 1, 2, 2);
    config.image_rect = (1, 1, 2, 2);
    services
        .create_pin_from_capture(image.clone(), config)
        .unwrap();
    services.shutdown();
    drop(services);
    let (pins, warnings) = rotor_screenshot::pin_store::PinStore::load_from(directory.path())
        .unwrap()
        .load_pins();
    assert!(warnings.is_empty());
    assert_eq!(pins.len(), 1);
    assert_eq!(
        *pins[0].image,
        image::imageops::crop_imm(image.as_ref(), 1, 1, 2, 2).to_image()
    );
    assert_eq!(pins[0].config.image_rect, (1, 1, 2, 2));
}

#[test]
fn pre_cropped_capture_keeps_monitor_origin_and_request_identity() {
    let directory = tempfile::tempdir().unwrap();
    let (services, events) = create(ConfigService::load_from(directory.path()).unwrap());
    let image = Arc::new(RgbaImage::from_pixel(2, 2, image::Rgba([12, 34, 56, 78])));
    let mut config = pin_config();
    config.monitor_size = (3840, 2160);
    config.rect = (1200, 900, 2, 2);
    config.image_rect = config.rect;
    let request = services.create_pin(image.clone(), config.clone()).unwrap();
    let event = services.runtime().block_on(async {
        tokio::time::timeout(Duration::from_secs(3), events.recv())
            .await
            .unwrap()
            .unwrap()
    });
    let RuntimeEvent::Pin(crate::PinEvent::Created {
        id,
        result: Ok(pin),
    }) = event
    else {
        panic!("expected created pin");
    };
    assert_eq!(id, request);
    assert!(Arc::ptr_eq(&pin.image, &image));
    assert_eq!(pin.config.image_rect, config.image_rect);
    services.shutdown();
    let (pins, warnings) = rotor_screenshot::pin_store::PinStore::load_from(directory.path())
        .unwrap()
        .load_pins();
    assert!(warnings.is_empty());
    assert_eq!(pins.len(), 1);
    assert_eq!(*pins[0].image, *image);
    assert_eq!(pins[0].config.image_rect, config.image_rect);
}

#[test]
fn exported_canvas_frame_keeps_its_viewport_size_and_pixels() {
    let directory = tempfile::tempdir().unwrap();
    let (services, events) = create(ConfigService::load_from(directory.path()).unwrap());
    let source = Arc::new(RgbaImage::from_pixel(2, 3, image::Rgba([20, 40, 80, 128])));
    services.create_pin(source.clone(), pin_config()).unwrap();
    let receive = || {
        services.runtime().block_on(async {
            tokio::time::timeout(Duration::from_secs(3), events.recv())
                .await
                .unwrap()
                .unwrap()
        })
    };
    let RuntimeEvent::Pin(crate::PinEvent::Created {
        result: Ok(pin), ..
    }) = receive()
    else {
        panic!("expected created pin");
    };
    let scene = rotor_canvas::Document::new(
        rotor_canvas::ImageSize {
            width: 2,
            height: 3,
        },
        rotor_canvas::ImageRect {
            x: 0,
            y: 0,
            width: 2,
            height: 3,
        },
    )
    .unwrap();
    let frame = services
        .runtime()
        .block_on(services.render_canvas(
            source,
            scene.scene().clone(),
            rotor_canvas::ImageSize {
                width: 4,
                height: 6,
            },
        ))
        .unwrap();
    let path = directory.path().join("scaled.png");
    let id = services
        .export_pin_frame(
            Some(pin.id),
            frame.clone(),
            crate::PinExportTarget::File(path.clone()),
        )
        .unwrap();
    assert!(
        matches!(receive(), RuntimeEvent::Pin(crate::PinEvent::Exported { id: returned, result: Ok(()) }) if returned == id)
    );
    let image = image::open(path).unwrap().into_rgba8();
    assert_eq!(image.dimensions(), (4, 6));
    assert_eq!(image, *frame);
}

#[test]
fn shutdown_wakes_canvas_requests_waiting_for_capacity() {
    use std::{
        future::Future,
        task::{Context, Waker},
    };
    let directory = tempfile::tempdir().unwrap();
    let (services, _events) = create(ConfigService::load_from(directory.path()).unwrap());
    let _permits: Vec<_> = (0..BACKGROUND_LIMIT)
        .map(|_| services.slots.clone().try_acquire_owned().unwrap())
        .collect();
    let scene = rotor_canvas::Document::new(
        rotor_canvas::ImageSize {
            width: 2,
            height: 3,
        },
        rotor_canvas::ImageRect {
            x: 0,
            y: 0,
            width: 2,
            height: 3,
        },
    )
    .unwrap();
    let mut request = Box::pin(services.render_canvas(
        Arc::new(RgbaImage::new(2, 3)),
        scene.scene().clone(),
        scene.scene().size,
    ));
    assert!(request
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
        .is_pending());
    services.shutdown();
    assert!(services.runtime().block_on(request).is_err());
}

#[test]
fn final_pin_snapshot_bypasses_full_command_and_event_queues() {
    let directory = tempfile::tempdir().unwrap();
    let (services, events) = create(ConfigService::load_from(directory.path()).unwrap());
    services
        .create_pin(Arc::new(RgbaImage::new(2, 3)), pin_config())
        .unwrap();
    let created = services.runtime().block_on(async {
        tokio::time::timeout(Duration::from_secs(3), events.recv())
            .await
            .unwrap()
            .unwrap()
    });
    let RuntimeEvent::Pin(crate::PinEvent::Created {
        result: Ok(pin), ..
    }) = created
    else {
        panic!("expected created pin");
    };
    let queued = EVENT_CAPACITY + services.pins.sender.capacity().unwrap() + 1;
    services.runtime().block_on(async {
        tokio::time::timeout(Duration::from_secs(5), async {
            for _ in 0..queued {
                services
                    .pins
                    .sender
                    .send(PinCommand::Restore {
                        id: next_operation(),
                        include_hidden: false,
                        excluded_ids: Vec::new(),
                    })
                    .await
                    .unwrap();
            }
        })
        .await
        .unwrap();
    });
    assert!(services.pins.sender.is_full());
    assert!(events.is_full());
    let mut latest = pin.config;
    latest.offset = (123, -321);
    services.shutdown_with_pin_updates(vec![(pin.id, latest)]);
    drop(services);
    let (pins, warnings) = rotor_screenshot::pin_store::PinStore::load_from(directory.path())
        .unwrap()
        .load_pins();
    assert!(warnings.is_empty());
    assert_eq!(pins[0].config.offset, (123, -321));
}

#[test]
fn failed_settings_write_requests_shortcut_rollback() {
    let directory = tempfile::tempdir().unwrap();
    let (services, events) = create(ConfigService::load_from(directory.path()).unwrap());
    let before = services.settings();
    services.coordinate_shortcuts(true);
    std::fs::create_dir(directory.path().join("config.toml")).unwrap();
    let id = services
        .save_settings(vec![("shortcut_search".into(), "Ctrl+Shift+X".into())])
        .unwrap();
    let receive = || {
        services.runtime().block_on(async {
            tokio::time::timeout(Duration::from_secs(3), events.recv())
                .await
                .unwrap()
                .unwrap()
        })
    };
    let RuntimeEvent::SettingsCoordination(SettingsCoordination::Prepare {
        id: staged,
        candidate,
        reply,
    }) = receive()
    else {
        panic!("expected shortcut preparation");
    };
    assert_eq!(id, staged);
    assert_eq!(candidate["shortcut_search"], "Ctrl+Shift+X");
    reply.send(Ok(())).unwrap();
    let RuntimeEvent::SettingsCoordination(SettingsCoordination::Finish {
        id: finished,
        committed,
        reply,
    }) = receive()
    else {
        panic!("expected rollback");
    };
    assert_eq!(id, finished);
    assert!(!committed);
    reply.send(Ok(())).unwrap();
    assert!(
        matches!(receive(), RuntimeEvent::SettingsSaved { id: result, result: Err(_) } if result == id)
    );
    assert_eq!(services.settings(), before);
}

#[test]
fn shortcut_prepare_rejection_does_not_touch_disk() {
    let directory = tempfile::tempdir().unwrap();
    let (services, events) = create(ConfigService::load_from(directory.path()).unwrap());
    services.coordinate_shortcuts(true);
    services
        .save_settings(vec![("shortcut_search".into(), "Ctrl+Shift+X".into())])
        .unwrap();
    let receive = || {
        services.runtime().block_on(async {
            tokio::time::timeout(Duration::from_secs(3), events.recv())
                .await
                .unwrap()
                .unwrap()
        })
    };
    let RuntimeEvent::SettingsCoordination(SettingsCoordination::Prepare { reply, .. }) = receive()
    else {
        panic!("expected shortcut preparation");
    };
    reply.send(Err("already occupied".into())).unwrap();
    assert!(
        matches!(receive(), RuntimeEvent::SettingsSaved { result: Err(error), .. } if error == "already occupied")
    );
    assert!(!directory.path().join("config.toml").exists());
}

#[test]
fn capture_uses_its_own_worker_when_background_capacity_is_saturated() {
    let directory = tempfile::tempdir().unwrap();
    let (mut services, _events) = create(ConfigService::load_from(directory.path()).unwrap());
    // Exercise the real Services submission route using synthetic pixels.
    services.capture_worker.stop();
    services.capture_worker =
        CaptureWorker::start(services.events.clone(), services.capture_id.clone(), |_| {
            Ok(CaptureBundle {
                monitors: Vec::new(),
                windows: Vec::new(),
            })
        })
        .unwrap();
    let _permit = services
        .slots
        .clone()
        .try_acquire_many_owned(BACKGROUND_LIMIT as u32)
        .unwrap();
    let id = services.capture().unwrap();
    assert_eq!(services.capture_id.load(Ordering::Acquire), id.0);
    services.shutdown();
    let cancelled = services.capture_id.load(Ordering::Acquire);
    assert!(services.capture().is_err());
    assert_eq!(services.capture_id.load(Ordering::Acquire), cancelled);
}

#[test]
fn translation_uses_isolated_configuration_and_keeps_request_identity() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && std::time::Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(error) => panic!("test HTTP server: {error}"),
            }
        };
        // Windows accepted sockets can inherit the listener's nonblocking
        // mode. Use bounded blocking I/O for the HTTP fixture itself.
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut request = Vec::new();
        let mut buffer = [0; 1024];
        while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            let count = stream.read(&mut buffer).unwrap();
            assert!(count > 0);
            request.extend_from_slice(&buffer[..count]);
        }
        assert!(String::from_utf8_lossy(&request).contains("text=hello%20world"));
        let body = "{\"translated\":\"你好\"}";
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    });
    let directory = tempfile::tempdir().unwrap();
    let mut config = ConfigService::load_from(directory.path()).unwrap();
    config
        .set_many([
            ("translator_engine".into(), "custom".into()),
            (
                "translator_custom_url".into(),
                format!("http://{address}/?text={{text}}"),
            ),
        ])
        .unwrap();
    let (services, events) = create(config);
    let id = services.translate("hello world".into()).unwrap();
    let event = services.runtime().block_on(async {
        tokio::time::timeout(Duration::from_secs(3), events.recv())
            .await
            .unwrap()
            .unwrap()
    });
    match event {
        RuntimeEvent::TranslationFinished {
            id: returned,
            result,
        } => {
            assert_eq!(returned, id);
            assert_eq!(result.unwrap().translated, "你好");
        }
        _ => panic!("expected completed custom translation"),
    }
    server.join().unwrap();
}
