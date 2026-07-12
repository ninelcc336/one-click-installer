use crate::{
    adapters::{cc_switch, codex, AppAdapter},
    coordinator::{EventSink, InstallCoordinator, InstallEvent},
    domain::{AppId, InstallError},
    download::DownloadManager,
    manifest::Manifest,
    platform,
};
use serde::Serialize;
use std::{env, fs, path::PathBuf, process::Command};
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
    pub install_dir: String,
    pub install_path_note: String,
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
                        .detect(&release, platform.as_ref(), &install_dir)
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
        install_dir: install_dir.to_string_lossy().into_owned(),
        install_path_note: if target.os == crate::domain::Os::Windows {
            "Codex 的 Microsoft Store 版本由系统管理安装位置；CC Switch 使用官方安装器。".to_owned()
        } else {
            "可将应用安装到你选择的 macOS Applications 目录。".to_owned()
        },
        apps,
    })
}

#[tauri::command]
pub fn start_install(
    app: AppHandle,
    selected_apps: Vec<AppId>,
    install_dir: String,
    state: State<'_, InstallerState>,
) -> Result<(), String> {
    if selected_apps.is_empty() {
        return Err("请至少选择一个应用".to_owned());
    }
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
        PathBuf::from(install_dir),
        adapters,
        Box::new(downloader),
        checkpoint_path,
    );
    let plan = coordinator
        .plan(&cache_dir, &selected_apps)
        .map_err(to_message)?;
    coordinator
        .run(&plan, &TauriEventSink { app })
        .map(|_| ())
        .map_err(to_message)
}

#[tauri::command]
pub fn open_log_folder() -> Result<String, String> {
    let path = installer_data_dir()?;
    fs::create_dir_all(&path).map_err(|error| error.to_string())?;
    let status = if cfg!(target_os = "windows") {
        Command::new("explorer.exe").arg(&path).status()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(&path).status()
    } else {
        return Err("不支持当前操作系统".to_owned());
    };
    status.map_err(|error| error.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn choose_install_dir() -> Result<String, String> {
    let output = if cfg!(target_os = "windows") {
        Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "$dialog=New-Object -ComObject Shell.Application; $folder=$dialog.BrowseForFolder(0,'选择安装目录',0,0); if ($folder) { $folder.Self.Path }",
            ])
            .output()
            .map_err(|error| error.to_string())?
    } else if cfg!(target_os = "macos") {
        Command::new("osascript")
            .args([
                "-e",
                "POSIX path of (choose folder with prompt \"选择安装目录\")",
            ])
            .output()
            .map_err(|error| error.to_string())?
    } else {
        return Err("不支持当前操作系统".to_owned());
    };
    if !output.status.success() {
        return Err("用户取消选择安装目录".to_owned());
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if path.is_empty() {
        return Err("用户取消选择安装目录".to_owned());
    }
    Ok(path)
}

#[tauri::command]
pub fn launch_app(app_id: AppId, state: State<'_, InstallerState>) -> Result<(), String> {
    let platform = platform::current().map_err(to_message)?;
    let release = state
        .manifest
        .select(app_id, platform.target())
        .map_err(to_message)?;
    let install_dir = platform.user_applications_dir().map_err(to_message)?;
    let adapter: Box<dyn crate::adapters::AppAdapter> = if app_id == AppId::Codex {
        Box::new(codex::adapter())
    } else {
        Box::new(cc_switch::adapter())
    };
    let installed = adapter
        .detect(&release, platform.as_ref(), &install_dir)
        .map_err(to_message)?
        .ok_or_else(|| format!("没有找到已安装的 {}", app_id))?;
    adapter
        .launch(&installed, platform.as_ref())
        .map_err(to_message)
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
