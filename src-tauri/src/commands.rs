use crate::{
    adapters::{cc_switch, codex, AppAdapter},
    coordinator::{EventSink, InstallCoordinator, InstallEvent},
    domain::{AppId, InstallError},
    download::DownloadManager,
    manifest::Manifest,
    platform,
};
use serde::Serialize;
use std::{env, path::PathBuf};
use tauri::{AppHandle, Emitter, State};

pub struct InstallerState {
    pub manifest: Manifest,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreflightApp {
    pub app_id: AppId,
    pub available: bool,
    pub installed: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreflightResult {
    pub target: String,
    pub free_space_bytes: u64,
    pub can_write: bool,
    pub apps: Vec<PreflightApp>,
}

#[derive(Clone)]
struct TauriEventSink {
    app: AppHandle,
}

impl EventSink for TauriEventSink {
    fn emit(&self, event: InstallEvent) {
        let _ = self.app.emit("install-event", event);
    }
}

#[tauri::command]
pub fn get_preflight(state: State<'_, InstallerState>) -> Result<PreflightResult, String> {
    let platform = platform::current().map_err(to_message)?;
    let target = platform.target();
    let install_dir = platform.user_applications_dir().map_err(to_message)?;
    let free_space_bytes = platform.free_space(&install_dir).map_err(to_message)?;
    let can_write = platform
        .has_install_permission(&install_dir)
        .map_err(to_message)?;
    let apps = [AppId::Codex, AppId::CcSwitch]
        .into_iter()
        .map(|app_id| {
            let release = state.manifest.select(app_id, target);
            match release {
                Ok(release) => {
                    let adapter = if app_id == AppId::Codex {
                        codex::adapter()
                    } else {
                        cc_switch::adapter()
                    };
                    let installed = adapter
                        .detect(&release, platform.as_ref())
                        .ok()
                        .flatten()
                        .is_some();
                    PreflightApp {
                        app_id,
                        available: true,
                        installed,
                        message: if installed {
                            "已检测到，可以保留现有版本".to_owned()
                        } else {
                            "可以安装".to_owned()
                        },
                    }
                }
                Err(_) => PreflightApp {
                    app_id,
                    available: false,
                    installed: false,
                    message: "等待核验安装包信息".to_owned(),
                },
            }
        })
        .collect();

    Ok(PreflightResult {
        target: target.key(),
        free_space_bytes,
        can_write,
        apps,
    })
}

#[tauri::command]
pub fn start_install(app: AppHandle, state: State<'_, InstallerState>) -> Result<(), String> {
    let platform = platform::current().map_err(to_message)?;
    let base_dir = installer_data_dir()?;
    let cache_dir = base_dir.join("packages");
    let checkpoint_path = base_dir.join("checkpoint.json");
    let adapters: Vec<Box<dyn crate::adapters::AppAdapter>> =
        vec![Box::new(codex::adapter()), Box::new(cc_switch::adapter())];
    let downloader = DownloadManager::new(3).map_err(to_message)?;
    let coordinator = InstallCoordinator::new(
        state.manifest.clone(),
        platform,
        adapters,
        Box::new(downloader),
        checkpoint_path,
    );
    let plan = coordinator.plan(&cache_dir).map_err(to_message)?;
    coordinator
        .run(&plan, &TauriEventSink { app })
        .map(|_| ())
        .map_err(to_message)
}

#[tauri::command]
pub fn open_log_folder() -> Result<String, String> {
    Ok(installer_data_dir()?.to_string_lossy().into_owned())
}

fn installer_data_dir() -> Result<PathBuf, String> {
    if cfg!(target_os = "windows") {
        env::var_os("LOCALAPPDATA")
            .map(|value| PathBuf::from(value).join("CodexCcSwitchInstaller"))
            .ok_or_else(|| "找不到 LOCALAPPDATA".to_owned())
    } else if cfg!(target_os = "macos") {
        env::var_os("HOME")
            .map(|value| {
                PathBuf::from(value).join("Library/Application Support/CodexCcSwitchInstaller")
            })
            .ok_or_else(|| "找不到 HOME".to_owned())
    } else {
        Err("不支持当前操作系统".to_owned())
    }
}

fn to_message(error: InstallError) -> String {
    error.to_string()
}
