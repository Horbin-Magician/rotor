use crate::bundle;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, String>;

#[derive(Deserialize, Serialize)]
struct Job {
    archive: PathBuf,
    signature: String,
    version: String,
    parent: u32,
    arguments: Vec<String>,
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())
}
fn app_directory() -> Result<PathBuf> {
    let executable = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let macos = executable.parent().ok_or("Executable has no directory")?;
    let contents = macos.parent().ok_or("App has no Contents directory")?;
    if macos.file_name().is_none_or(|name| name != "MacOS")
        || contents.file_name().is_none_or(|name| name != "Contents")
    {
        return Err("Install updates from a packaged Rotor app".into());
    }
    Ok(contents
        .parent()
        .ok_or("App directory missing")?
        .to_path_buf())
}
fn validate_arguments(arguments: &[String]) -> Result<()> {
    if arguments.len() < 2 || arguments[0] != "--data-dir" || !arguments[1].starts_with('/') {
        return Err("Update restart requires an absolute profile directory".into());
    }
    if arguments.iter().any(|argument| argument.contains('\0'))
        || arguments[2..].iter().any(|argument| {
            !matches!(
                argument.as_str(),
                "--no-index" | "--no-hotkeys" | "--production-shortcuts" | "--no-elevate"
            )
        })
    {
        return Err("Unsupported update restart arguments".into());
    }
    Ok(())
}
#[cfg(target_os = "macos")]
fn alive(pid: u32) -> Result<bool> {
    if pid == 0 || pid > i32::MAX as u32 || pid == std::process::id() {
        return Err("Invalid update parent process".into());
    }
    if unsafe { libc::kill(pid as i32, 0) } == 0 {
        return Ok(true);
    }
    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::ESRCH) => Ok(false),
        Some(libc::EPERM) => Ok(true),
        _ => Err("Cannot query update parent process".into()),
    }
}
// Type-check the portable handoff logic on other test hosts without emulating
// macOS process control or running an installer.
#[cfg(all(test, not(target_os = "macos")))]
fn alive(_pid: u32) -> Result<bool> {
    Err("macOS process waiting is unavailable on this test host".into())
}
fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

pub fn handoff_error(path: &Path) -> Result<Option<String>> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > 1024 * 1024 {
        return Err("Update receipt is too large".into());
    }
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    Ok(value["error"].as_str().map(str::to_string))
}

/// Called by the live app on a worker. It quits only after helper preflight succeeds.
pub fn launch_handoff(
    archive: &Path,
    signature: &str,
    version: &str,
    profile: &Path,
    flags: &[String],
) -> Result<()> {
    let app = app_directory()?;
    let info = bundle::inspect(&app)?;
    if semver::Version::parse(version).map_err(|e| e.to_string())? <= info.version {
        return Err("Update would not advance the installed version".into());
    }
    let mut arguments = vec![
        "--data-dir".into(),
        profile
            .to_str()
            .ok_or("Profile path is not Unicode")?
            .into(),
    ];
    arguments.extend(
        flags
            .iter()
            .filter(|flag| {
                matches!(
                    flag.as_str(),
                    "--no-index" | "--no-hotkeys" | "--production-shortcuts" | "--no-elevate"
                )
            })
            .cloned(),
    );
    validate_arguments(&arguments)?;
    let root = tempfile::Builder::new()
        .prefix("handoff-")
        .tempdir_in(archive.parent().ok_or("Archive has no directory")?)
        .map_err(|e| e.to_string())?;
    let job = Job {
        archive: archive.canonicalize().map_err(|e| e.to_string())?,
        signature: signature.into(),
        version: version.into(),
        parent: std::process::id(),
        arguments,
    };
    let job_path = root.path().join("job.json");
    write_new(
        &job_path,
        &serde_json::to_vec(&job).map_err(|e| e.to_string())?,
    )?;
    let log = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.path().join("helper.log"))
        .map_err(|e| e.to_string())?;
    let mut child = Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
        .arg("--apply-update")
        .arg(job_path)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone().map_err(|e| e.to_string())?))
        .stderr(Stdio::from(log))
        .spawn()
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        let status = match child.try_wait() {
            Ok(status) => status,
            Err(error) => {
                stop(&mut child);
                let _ = root.keep();
                return Err(error.to_string());
            }
        };
        if let Some(status) = status {
            let result = fs::read_to_string(root.path().join("result.json"))
                .ok()
                .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
                .and_then(|value| value["error"].as_str().map(str::to_string))
                .unwrap_or_else(|| format!("Update helper stopped before preparation: {status}"));
            let _ = root.keep();
            return Err(result);
        }
        if root.path().join("ready").is_file() {
            let _ = root.keep();
            return Ok(());
        }
        if Instant::now() >= deadline {
            stop(&mut child);
            let _ = root.keep();
            return Err("Update preparation timed out; current app remains running".into());
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn launch_and_wait(app: &Path, arguments: &[String], root: &Path) -> Result<()> {
    let executable = bundle::inspect(app)?.executable;
    let acknowledgement = root.join("new-app-ready");
    if acknowledgement.exists() {
        return Err("Startup acknowledgement already exists".into());
    }
    let log = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join("new-app.log"))
        .map_err(|e| e.to_string())?;
    let mut child = Command::new(executable)
        .args(arguments)
        .arg("--update-ready")
        .arg(&acknowledgement)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone().map_err(|e| e.to_string())?))
        .stderr(Stdio::from(log))
        .spawn()
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let status = match child.try_wait() {
            Ok(status) => status,
            Err(error) => {
                stop(&mut child);
                return Err(error.to_string());
            }
        };
        if let Some(status) = status {
            return Err(format!("New app exited during startup: {status}"));
        }
        if acknowledgement.is_file() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            stop(&mut child);
            return Err("New app did not acknowledge startup".into());
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// Runs before GPUI initialization in a helper process executing the old binary.
pub fn run_helper(job_path: &Path) -> Result<()> {
    if fs::metadata(job_path).map_err(|e| e.to_string())?.len() > 1024 * 1024 {
        return Err("Update job is too large".into());
    }
    let job: Job = serde_json::from_slice(&fs::read(job_path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    validate_arguments(&job.arguments)?;
    let root = job_path
        .parent()
        .ok_or("Update job directory is missing")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let app = app_directory()?;
    let previous = bundle::inspect(&app)?;
    let parent = app.parent().ok_or("App has no parent")?;
    let suffix = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Invalid handoff directory")?;
    if !suffix.starts_with("handoff-") {
        return Err("Invalid handoff directory".into());
    }
    let backup = parent.join(format!(".rotor-backup-{suffix}.app"));
    let failed = parent.join(format!(".rotor-failed-{suffix}.app"));
    let mut parent_exited = false;
    let result = (|| {
        if semver::Version::parse(&job.version).map_err(|e| e.to_string())? <= previous.version {
            return Err("Update is not newer than the current app".into());
        }
        let prepared = bundle::prepare(
            &job.archive,
            &job.signature,
            &job.version,
            &previous.identifier,
            parent,
        )?;
        if !alive(job.parent)? {
            return Err("Parent exited before update preparation completed".into());
        }
        write_new(&root.join("ready"), b"prepared")?;
        let deadline = Instant::now() + Duration::from_secs(30);
        while alive(job.parent)? {
            if Instant::now() >= deadline {
                return Err("Current app did not exit; update not installed".into());
            }
            thread::sleep(Duration::from_millis(100));
        }
        parent_exited = true;
        bundle::replace_and_launch(&prepared.app, &app, &backup, &failed, |app| {
            launch_and_wait(app, &job.arguments, &root)
        })
    })();
    let receipt = serde_json::json!({"version": job.version, "success": result.is_ok(), "error": result.as_ref().err(), "backup": backup, "failed_bundle": failed});
    let _ = write_new(
        &root.join("result.json"),
        &serde_json::to_vec_pretty(&receipt).map_err(|e| e.to_string())?,
    );
    if result.is_err()
        && parent_exited
        && bundle::inspect(&app).is_ok_and(|info| info.version == previous.version)
    {
        let _ = Command::new(app.join("Contents/MacOS/rotor-desktop"))
            .args(&job.arguments)
            .arg("--update-error-file")
            .arg(root.join("result.json"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
    result
}

#[cfg(test)]
mod tests {
    #[test]
    fn restart_arguments_cannot_invoke_another_helper() {
        assert!(super::validate_arguments(&[
            "--data-dir".into(),
            "/Users/me/Rotor Data".into(),
            "--no-index".into()
        ])
        .is_ok());
        assert!(super::validate_arguments(&[
            "--data-dir".into(),
            "/tmp/profile".into(),
            "--apply-update".into()
        ])
        .is_err());
        assert!(super::validate_arguments(&["--data-dir".into(), "relative".into()]).is_err());
    }
}
