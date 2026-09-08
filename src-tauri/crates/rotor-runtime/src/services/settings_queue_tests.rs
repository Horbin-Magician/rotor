use super::*;

fn setup() -> (
    tempfile::TempDir,
    Arc<Mutex<ConfigService>>,
    Services,
    Receiver<RuntimeEvent>,
) {
    let directory = tempfile::tempdir().unwrap();
    let config = Arc::new(Mutex::new(
        ConfigService::load_from(directory.path()).unwrap(),
    ));
    let (services, events) =
        Services::new(config.clone(), None, ServiceOptions { index_files: false }).unwrap();
    (directory, config, services, events)
}

fn patch(value: &str) -> Vec<(String, String)> {
    vec![("fixture_value".into(), value.into())]
}

#[test]
fn rapid_fields_share_one_receipt_and_keep_latest_values() {
    let (_directory, config, services, events) = setup();
    let hold = lock(&config);
    let first = services
        .save_settings(vec![("fixture_gate".into(), "hold".into())])
        .unwrap();
    let batch = services.save_settings_coalesced(patch("0")).unwrap();
    for index in 1..1000 {
        assert_eq!(
            services
                .save_settings_coalesced(patch(&index.to_string()))
                .unwrap(),
            batch
        );
    }
    assert_eq!(
        services
            .save_settings_coalesced(vec![("fixture_other".into(), "second field".into())])
            .unwrap(),
        batch
    );
    drop(hold);
    services
        .runtime()
        .block_on(services.flush_settings())
        .unwrap();
    let receipts = [
        events.recv_blocking().unwrap(),
        events.recv_blocking().unwrap(),
    ];
    for (event, expected) in receipts.into_iter().zip([first, batch]) {
        match event {
            RuntimeEvent::SettingsSaved { id, result } => {
                assert_eq!(id, expected);
                assert!(result.is_ok());
            }
            _ => panic!("expected a settings receipt"),
        }
    }
    assert_eq!(services.settings()["fixture_value"], "999");
    assert_eq!(services.settings()["fixture_other"], "second field");
    assert!(events.try_recv().is_err());
}

#[test]
fn explicit_save_separates_automatic_batches_in_submission_order() {
    let (_directory, config, services, events) = setup();
    let hold = lock(&config);
    services
        .save_settings(vec![("fixture_gate".into(), "hold".into())])
        .unwrap();
    let before = services.save_settings_coalesced(patch("before")).unwrap();
    let manual = services.save_settings(patch("manual")).unwrap();
    let after = services.save_settings_coalesced(patch("after")).unwrap();
    assert_ne!(before, after);
    drop(hold);
    services
        .runtime()
        .block_on(services.flush_settings())
        .unwrap();
    let _gate = events.recv_blocking().unwrap();
    for (expected, value) in [(before, "before"), (manual, "manual"), (after, "after")] {
        match events.recv_blocking().unwrap() {
            RuntimeEvent::SettingsSaved { id, result } => {
                assert_eq!(id, expected);
                assert_eq!(result.unwrap()["fixture_value"], value);
            }
            _ => panic!("expected an ordered settings receipt"),
        }
    }
}

#[test]
fn accepted_automatic_values_survive_service_shutdown_without_a_view() {
    let (directory, config, services, _events) = setup();
    let hold = lock(&config);
    services
        .save_settings(vec![("fixture_gate".into(), "hold".into())])
        .unwrap();
    services.save_settings_coalesced(patch("first")).unwrap();
    services.save_settings_coalesced(patch("last")).unwrap();
    services.shutdown();
    assert!(services.save_settings_coalesced(patch("too late")).is_err());
    drop(hold);
    drop(services);
    assert_eq!(
        ConfigService::load_from(directory.path())
            .unwrap()
            .get_all()["fixture_value"],
        "last"
    );
}

#[test]
fn rejected_merge_cannot_change_already_accepted_values() {
    let (_directory, config, services, _events) = setup();
    let hold = lock(&config);
    services
        .save_settings(vec![("fixture_gate".into(), "hold".into())])
        .unwrap();
    let changes = (0..64)
        .map(|id| (format!("fixture_{id}"), "accepted".into()))
        .collect();
    let batch = services.save_settings_coalesced(changes).unwrap();
    assert!(services
        .save_settings_coalesced(vec![
            ("fixture_0".into(), "must not replace".into()),
            ("overflow".into(), "extra".into())
        ])
        .is_err());
    assert_eq!(
        services
            .save_settings_coalesced(vec![("fixture_1".into(), "replacement".into())])
            .unwrap(),
        batch
    );
    drop(hold);
    services
        .runtime()
        .block_on(services.flush_settings())
        .unwrap();
    assert_eq!(services.settings()["fixture_0"], "accepted");
    assert_eq!(services.settings()["fixture_1"], "replacement");
    assert!(!services.settings().contains_key("overflow"));
}

#[test]
fn a_full_queue_can_still_replace_its_automatic_tail() {
    let (_directory, config, services, _events) = setup();
    let hold = lock(&config);
    services
        .save_settings(vec![("fixture_gate".into(), "hold".into())])
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while !services.settings.is_empty() {
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    // The first write is now waiting for the held config lock.
    for _ in 0..SETTINGS_CAPACITY - 1 {
        services.save_settings(patch("manual")).unwrap();
    }
    let last = services
        .save_settings_coalesced(patch("first automatic"))
        .unwrap();
    assert!(services.save_settings(patch("rejected manual")).is_err());
    assert_eq!(
        services
            .save_settings_coalesced(patch("latest automatic"))
            .unwrap(),
        last
    );
    drop(hold);
    services
        .runtime()
        .block_on(services.flush_settings())
        .unwrap();
    assert_eq!(services.settings()["fixture_value"], "latest automatic");
}

#[test]
fn automatic_shortcut_save_is_not_complete_until_the_ui_confirms_the_transaction() {
    let (_directory, _config, services, events) = setup();
    services.coordinate_shortcuts(true);
    let old = services.settings()["shortcut_search"].clone();
    let id = services
        .save_settings_coalesced(vec![("shortcut_search".into(), "Ctrl+Shift+X".into())])
        .unwrap();
    let receive = || {
        services.runtime().block_on(async {
            tokio::time::timeout(Duration::from_secs(3), events.recv())
                .await
                .unwrap()
                .unwrap()
        })
    };
    match receive() {
        RuntimeEvent::SettingsCoordination(SettingsCoordination::Prepare {
            id: received,
            candidate,
            reply,
        }) => {
            assert_eq!(received, id);
            assert_eq!(candidate["shortcut_search"], "Ctrl+Shift+X");
            assert_eq!(services.settings()["shortcut_search"], old);
            assert!(events.try_recv().is_err());
            reply.send(Ok(())).unwrap();
        }
        _ => panic!("expected UI shortcut preparation"),
    }
    match receive() {
        RuntimeEvent::SettingsCoordination(SettingsCoordination::Finish {
            id: received,
            committed,
            reply,
        }) => {
            assert_eq!(received, id);
            assert!(committed);
            assert!(events.try_recv().is_err());
            reply.send(Ok(())).unwrap();
        }
        _ => panic!("expected UI transaction completion"),
    }
    match receive() {
        RuntimeEvent::SettingsSaved {
            id: received,
            result,
        } => {
            assert_eq!(received, id);
            assert_eq!(result.unwrap()["shortcut_search"], "Ctrl+Shift+X");
        }
        _ => panic!("expected save receipt after both acknowledgements"),
    }
}

#[test]
fn local_shortcut_validation_is_atomic_and_allows_explicit_disabling() {
    let (_directory, _config, services, events) = setup();
    let before = services.settings();
    services
        .save_settings_coalesced(vec![
            ("shortcut_pinwin_save".into(), "Ctrl+".into()),
            ("fixture_value".into(), "must not commit".into()),
        ])
        .unwrap();
    let receive = || {
        services.runtime().block_on(async {
            tokio::time::timeout(Duration::from_secs(3), events.recv())
                .await
                .unwrap()
                .unwrap()
        })
    };
    assert!(matches!(
        receive(),
        RuntimeEvent::SettingsSaved { result: Err(_), .. }
    ));
    assert_eq!(services.settings(), before);
    for (value, expected) in [("  Ctrl+KeyS  ", "Ctrl+KeyS"), ("  ", "")] {
        services
            .save_settings_coalesced(vec![("shortcut_pinwin_save".into(), value.into())])
            .unwrap();
        match receive() {
            RuntimeEvent::SettingsSaved {
                result: Ok(snapshot),
                ..
            } => assert_eq!(snapshot["shortcut_pinwin_save"], expected),
            _ => panic!("expected normalized local shortcut save"),
        }
    }
}

#[cfg(windows)]
#[test]
fn locked_config_rolls_back_an_automatic_batch_and_allows_retry_after_unlock() {
    use std::{fs, os::windows::fs::OpenOptionsExt};

    let (directory, config, services, events) = setup();
    let receive = || {
        services.runtime().block_on(async {
            tokio::time::timeout(Duration::from_secs(3), events.recv())
                .await
                .unwrap()
                .unwrap()
        })
    };
    services
        .save_settings(vec![("fixture_unknown".into(), "保留旧值".into())])
        .unwrap();
    assert!(matches!(
        receive(),
        RuntimeEvent::SettingsSaved { result: Ok(_), .. }
    ));
    let path = directory.path().join("config.toml");
    let before_bytes = fs::read(&path).unwrap();
    let before = services.settings();
    // Permit reads but deny write/delete sharing, so the real Windows rename
    // fails after the temporary candidate has been written and flushed.
    let mut occupied = Some(
        fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&path)
            .unwrap(),
    );
    services.coordinate_shortcuts(true);
    let mut failed_id = None;
    for should_commit in [false, true] {
        if should_commit {
            drop(occupied.take());
        }
        let id = services
            .save_settings_coalesced(vec![
                ("shortcut_search".into(), "Ctrl+Shift+X".into()),
                ("fixture_text".into(), "自动保存 中文".into()),
            ])
            .unwrap();
        assert_ne!(Some(id), failed_id);
        let RuntimeEvent::SettingsCoordination(SettingsCoordination::Prepare {
            id: received,
            candidate,
            reply,
        }) = receive()
        else {
            panic!("expected automatic batch preparation");
        };
        assert_eq!(received, id);
        assert_eq!(candidate["fixture_text"], "自动保存 中文");
        assert_eq!(services.settings(), before);
        reply.send(Ok(())).unwrap();
        let RuntimeEvent::SettingsCoordination(SettingsCoordination::Finish {
            id: received,
            committed,
            reply,
        }) = receive()
        else {
            panic!("expected commit or rollback request");
        };
        assert_eq!(received, id);
        assert_eq!(committed, should_commit);
        assert!(events.try_recv().is_err());
        reply.send(Ok(())).unwrap();
        let RuntimeEvent::SettingsSaved {
            id: received,
            result,
        } = receive()
        else {
            panic!("expected automatic save receipt after coordination");
        };
        assert_eq!(received, id);
        if should_commit {
            let saved = result.unwrap();
            assert_eq!(saved["shortcut_search"], "Ctrl+Shift+X");
            assert_eq!(saved["fixture_text"], "自动保存 中文");
            assert_eq!(saved["fixture_unknown"], "保留旧值");
            assert_eq!(services.settings(), saved);
            assert_eq!(
                ConfigService::load_from(directory.path())
                    .unwrap()
                    .get_all(),
                saved
            );
        } else {
            assert!(result.is_err());
            assert_eq!(fs::read(&path).unwrap(), before_bytes);
            assert_eq!(lock(&config).get_all(), before);
            assert_eq!(services.settings(), before);
            failed_id = Some(id);
        }
        assert!(fs::read_dir(directory.path()).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".config.toml.")
        }));
    }
}
