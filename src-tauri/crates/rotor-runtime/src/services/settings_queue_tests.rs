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
