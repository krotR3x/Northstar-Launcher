use crate::config::LAUNCHER_DIRECTORY;
use crate::error::{AppError, CommandError};
use crate::state::State;
use serde::Serialize;
use std::collections::HashMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use uuid::Uuid;

type Result<T> = std::result::Result<T, CommandError>;
type SharedChild = Arc<Mutex<Child>>;

static HORIZON_PROCESSES: OnceLock<Mutex<HashMap<Uuid, SharedChild>>> = OnceLock::new();

fn process_registry() -> &'static Mutex<HashMap<Uuid, SharedChild>> {
    HORIZON_PROCESSES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn invalid_input(message: impl Into<String>) -> CommandError {
    CommandError::from(AppError::Other(message.into()))
}

#[derive(Debug, Clone, Serialize)]
pub struct HorizonProcessStatus {
    pub process_id: Uuid,
    pub pid: Option<u32>,
    pub state: &'static str,
    pub exit_code: Option<i32>,
}

fn canonical_executable(path: &Path) -> Result<PathBuf> {
    let path = fs::canonicalize(path)
        .map_err(|e| invalid_input(format!("Cannot resolve the Horizon executable: {e}")))?;
    let metadata = fs::metadata(&path)
        .map_err(|e| invalid_input(format!("Cannot inspect the Horizon executable: {e}")))?;
    if !metadata.is_file() {
        return Err(invalid_input(
            "The Horizon executable must be a regular file",
        ));
    }
    let executable_name = path.file_name().and_then(|name| name.to_str());
    if !executable_name.is_some_and(|name| {
        name == "horizon-client" || name.eq_ignore_ascii_case("horizon-client.exe")
    }) {
        return Err(invalid_input(
            "Select Horizon's official horizon-client executable",
        ));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(invalid_input(
                "The Horizon file is not executable; check its execute permission",
            ));
        }
    }

    #[cfg(windows)]
    if !path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
    {
        return Err(invalid_input("The Horizon executable must be an .exe file"));
    }

    Ok(path)
}

fn canonical_resources(path: &Path) -> Result<PathBuf> {
    let path = fs::canonicalize(path)
        .map_err(|e| invalid_input(format!("Cannot resolve Horizon's prepared resources: {e}")))?;
    if !fs::metadata(&path)
        .map_err(|e| invalid_input(format!("Cannot inspect Horizon resources: {e}")))?
        .is_dir()
    {
        return Err(invalid_input("Horizon resources must be a directory"));
    }
    Ok(path)
}

fn validate_world_name(name: &str) -> Result<()> {
    let valid = !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '-'));
    if valid {
        Ok(())
    } else {
        Err(invalid_input(
            "World name must be 1–64 characters using letters, numbers, spaces, '_' or '-'",
        ))
    }
}

fn build_horizon_args(
    resources: &Path,
    world: Option<&str>,
    render_distance: Option<u8>,
) -> Result<Vec<String>> {
    let mut args = vec![
        "--resources".to_string(),
        resources.to_string_lossy().into_owned(),
    ];
    if let Some(world) = world {
        validate_world_name(world)?;
        args.push("--world".to_string());
        args.push(world.to_string());
    }
    if let Some(distance) = render_distance {
        if !(2..=64).contains(&distance) {
            return Err(invalid_input(
                "Render distance must be between 2 and 64 chunks",
            ));
        }
        args.push("--render-distance".to_string());
        args.push(distance.to_string());
    }
    Ok(args)
}

/// Save paths explicitly selected by the user. Paths are revalidated at launch time.
#[tauri::command]
pub async fn configure_horizon(executable_path: String, resources_path: String) -> Result<()> {
    let executable = canonical_executable(Path::new(&executable_path))?;
    let resources = canonical_resources(Path::new(&resources_path))?;
    let state = State::get().await?;
    let mut config = state.config_manager.get_config().await;
    config.horizon_executable = Some(executable);
    config.horizon_resources = Some(resources);
    state.config_manager.set_config(config).await?;
    Ok(())
}

/// Launch Horizon with a constrained set of documented native-client arguments.
/// The current upstream engine does not expose a verified Ely.by token interface.
#[tauri::command]
pub async fn launch_horizon(
    app: AppHandle,
    world: Option<String>,
    render_distance: Option<u8>,
) -> Result<HorizonProcessStatus> {
    if let Some(world) = world.as_deref() {
        validate_world_name(world)?;
    }
    if let Some(distance) = render_distance {
        if !(2..=64).contains(&distance) {
            return Err(invalid_input(
                "Render distance must be between 2 and 64 chunks",
            ));
        }
    }

    let state = State::get().await?;
    let config = state.config_manager.get_config().await;
    let executable = config
        .horizon_executable
        .as_deref()
        .ok_or_else(|| invalid_input("Choose the Horizon executable in launcher settings first"))
        .and_then(canonical_executable)?;
    let resources = config
        .horizon_resources
        .as_deref()
        .ok_or_else(|| invalid_input("Choose Horizon's prepared resources directory first"))
        .and_then(canonical_resources)?;
    let args = build_horizon_args(&resources, world.as_deref(), render_distance)?;

    let log_dir = LAUNCHER_DIRECTORY.data_dir().join("logs");
    fs::create_dir_all(&log_dir)
        .map_err(|e| invalid_input(format!("Cannot create Horizon log directory: {e}")))?;
    let output = File::create(log_dir.join("horizon.log"))
        .map_err(|e| invalid_input(format!("Cannot create Horizon log file: {e}")))?;
    let error_output = output
        .try_clone()
        .map_err(|e| invalid_input(format!("Cannot open Horizon error log: {e}")))?;

    let mut command = Command::new(&executable);
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(output))
        .stderr(Stdio::from(error_output))
        .kill_on_drop(true);
    let child = command.spawn().map_err(|e| {
        invalid_input(format!(
            "Failed to start the configured Horizon client: {e}"
        ))
    })?;
    let pid = child.id();
    let process_id = Uuid::new_v4();
    let shared_child = Arc::new(Mutex::new(child));
    process_registry()
        .lock()
        .await
        .insert(process_id, Arc::clone(&shared_child));

    let status = HorizonProcessStatus {
        process_id,
        pid,
        state: "running",
        exit_code: None,
    };
    let _ = app.emit("horizon-process-state", &status);

    let app_handle = app.clone();
    tokio::spawn(async move {
        loop {
            let exit_status = {
                let mut child = shared_child.lock().await;
                child.try_wait()
            };
            match exit_status {
                Ok(Some(exit)) => {
                    process_registry().lock().await.remove(&process_id);
                    let finished = HorizonProcessStatus {
                        process_id,
                        pid,
                        state: if exit.success() { "exited" } else { "failed" },
                        exit_code: exit.code(),
                    };
                    let _ = app_handle.emit("horizon-process-state", &finished);
                    break;
                }
                Ok(None) => tokio::time::sleep(Duration::from_millis(400)).await,
                Err(error) => {
                    log::error!("Could not read Horizon process status: {error}");
                    process_registry().lock().await.remove(&process_id);
                    let failed = HorizonProcessStatus {
                        process_id,
                        pid,
                        state: "failed",
                        exit_code: None,
                    };
                    let _ = app_handle.emit("horizon-process-state", &failed);
                    break;
                }
            }
        }
    });

    Ok(status)
}

#[tauri::command]
pub async fn get_horizon_process(process_id: Uuid) -> Result<HorizonProcessStatus> {
    let child = process_registry()
        .lock()
        .await
        .get(&process_id)
        .cloned()
        .ok_or_else(|| invalid_input("No running Horizon process has that id"))?;
    let pid = child.lock().await.id();
    let exit = child
        .lock()
        .await
        .try_wait()
        .map_err(|e| invalid_input(format!("Could not read Horizon process status: {e}")))?;
    if let Some(exit) = exit {
        process_registry().lock().await.remove(&process_id);
        return Ok(HorizonProcessStatus {
            process_id,
            pid,
            state: if exit.success() { "exited" } else { "failed" },
            exit_code: exit.code(),
        });
    }
    Ok(HorizonProcessStatus {
        process_id,
        pid,
        state: "running",
        exit_code: None,
    })
}

#[tauri::command]
pub async fn stop_horizon(app: AppHandle, process_id: Uuid) -> Result<()> {
    let child = process_registry()
        .lock()
        .await
        .get(&process_id)
        .cloned()
        .ok_or_else(|| invalid_input("No running Horizon process has that id"))?;
    child
        .lock()
        .await
        .start_kill()
        .map_err(|e| invalid_input(format!("Failed to stop Horizon: {e}")))?;
    let _ = app.emit(
        "horizon-process-state",
        &HorizonProcessStatus {
            process_id,
            pid: None,
            state: "stopping",
            exit_code: None,
        },
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{build_horizon_args, canonical_executable, validate_world_name};
    use std::fs;

    #[test]
    fn horizon_args_are_individual_and_validated() {
        let args = build_horizon_args(
            std::path::Path::new("/tmp/horizon data"),
            Some("Survival World"),
            Some(12),
        )
        .expect("valid options should produce arguments");
        assert_eq!(args[0], "--resources");
        assert_eq!(args[1], "/tmp/horizon data");
        assert_eq!(
            &args[2..],
            &["--world", "Survival World", "--render-distance", "12"]
        );
    }

    #[test]
    fn horizon_world_name_cannot_escape_world_directory() {
        assert!(validate_world_name("../outside").is_err());
        assert!(validate_world_name("valid_world-1").is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn executable_path_must_be_executable_regular_file() {
        use std::os::unix::fs::PermissionsExt;
        let directory =
            std::env::temp_dir().join(format!("northstar-horizon-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory).expect("create fixture directory");
        let path = directory.join("horizon-client");
        fs::write(&path, "not a real executable").expect("create fixture");
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(&path, permissions).expect("set permissions");
        assert!(canonical_executable(&path).is_err());
        let _ = fs::remove_dir_all(directory);
    }
}
