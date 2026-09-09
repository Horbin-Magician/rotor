//! Separate executable: a broken GUI loader cannot run its own recovery code.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "windows")]
mod windows {
    use std::{
        path::Path,
        process::{Child, Command},
        time::{Duration, Instant},
    };

    fn monitor(child: &mut Child, ready: &Path, timeout: Duration) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        let mut acknowledged = None;
        loop {
            if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
                return Err(format!("New application exited during startup: {status}"));
            }
            if std::fs::read(ready).is_ok_and(|bytes| bytes == b"native-startup-ready") {
                let since = acknowledged.get_or_insert_with(Instant::now);
                if since.elapsed() >= Duration::from_secs(1) {
                    return Ok(());
                }
            }
            if Instant::now() >= deadline {
                child.kill().map_err(|error| error.to_string())?;
                child.wait().map_err(|error| error.to_string())?;
                return Err(
                    "New application did not acknowledge startup before the deadline".into(),
                );
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    pub fn run() -> Result<(), String> {
        let mut forwarded = Vec::new();
        let mut profile = None;
        let mut args = std::env::args_os().skip(1);
        while let Some(argument) = args.next() {
            match argument.to_str() {
                Some("--data-dir") => {
                    if profile.is_some() {
                        return Err("Duplicate update profile".into());
                    }
                    let value = args.next().ok_or("Missing explicit update profile")?;
                    profile = Some(std::path::PathBuf::from(&value));
                    forwarded.extend([argument, value]);
                }
                Some(
                    "--no-elevate"
                    | "--no-index"
                    | "--no-hotkeys"
                    | "--production-shortcuts"
                    | "--background",
                ) => forwarded.push(argument),
                _ => return Err("Unsupported recovery launcher argument".into()),
            }
        }
        let profile = profile
            .or_else(|| std::env::var_os("ROTOR_DATA_DIR").map(std::path::PathBuf::from))
            .unwrap_or_else(|| {
                std::env::home_dir()
                    .unwrap_or_default()
                    .join(rotor_common::native_app::PROFILE_DIRECTORY)
            });
        if !profile.is_absolute() {
            return Err("Recovery requires an absolute profile path".into());
        }
        let helper = std::env::current_exe().map_err(|error| error.to_string())?;
        let directory = helper
            .parent()
            .ok_or("Recovery launcher has no directory")?;
        let binary = directory.join(format!("{}.exe", rotor_common::native_app::EXECUTABLE_NAME));
        let acknowledgement = tempfile::tempdir().map_err(|error| error.to_string())?;
        let ready = acknowledgement.path().join("ready");
        let launched = Command::new(binary)
            .args(&forwarded)
            .arg("--update-ready")
            .arg(&ready)
            .spawn();
        let outcome = match launched {
            Ok(mut child) => monitor(&mut child, &ready, Duration::from_secs(30)),
            Err(error) => Err(format!("New application could not load: {error}")),
        };
        if let Err(error) = outcome {
            let flags = forwarded
                .iter()
                .filter_map(|arg| arg.to_str().map(str::to_owned))
                .collect::<Vec<_>>();
            // The uninstaller waits for this helper PID before moving its files.
            // It independently validates the registered sibling backup location.
            rotor_platform::desktop::rollback_failed_install(&profile, &flags)
                .map_err(|rollback| format!("{error}; rollback could not start: {rollback}"))?;
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn child_fixture() {
            match std::env::var("ROTOR_RECOVERY_FIXTURE").as_deref() {
                Ok("exit") => std::process::exit(23),
                Ok("ready" | "ready-exit") => {
                    std::fs::write(
                        std::env::var_os("ROTOR_RECOVERY_READY").unwrap(),
                        b"native-startup-ready",
                    )
                    .unwrap();
                    if std::env::var("ROTOR_RECOVERY_FIXTURE").as_deref() == Ok("ready-exit") {
                        return;
                    }
                }
                Ok("hang") => (),
                _ => return,
            }
            std::thread::sleep(Duration::from_secs(10));
        }

        #[test]
        fn independent_monitor_detects_exit_and_hang_and_accepts_readiness() {
            use std::os::windows::process::CommandExt;
            let directory = tempfile::tempdir().unwrap();
            for mode in ["exit", "hang", "ready-exit", "ready"] {
                let ready = directory.path().join(mode);
                let mut child = Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", "windows::tests::child_fixture", "--nocapture"])
                    .env("ROTOR_RECOVERY_FIXTURE", mode)
                    .env("ROTOR_RECOVERY_READY", &ready)
                    .creation_flags(0x08000000)
                    .spawn()
                    .unwrap();
                let result = monitor(&mut child, &ready, Duration::from_secs(2));
                assert_eq!(result.is_ok(), mode == "ready", "{mode}: {result:?}");
                if mode == "ready" {
                    child.kill().unwrap();
                    child.wait().unwrap();
                } else {
                    assert!(child.try_wait().unwrap().is_some());
                }
            }
            assert!(
                Command::new(directory.path().join("missing.exe"))
                    .spawn()
                    .is_err()
            );
        }
    }
}

fn main() {
    #[cfg(target_os = "windows")]
    if let Err(error) = windows::run() {
        rotor_platform::desktop::show_startup_error(&error);
        std::process::exit(1);
    }
}
