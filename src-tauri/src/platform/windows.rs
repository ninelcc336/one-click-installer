use super::{target_from_cfg, PlatformBackend};
use crate::domain::{AppId, InstallError, PlatformTarget};
use fs2::available_space;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

pub struct WindowsPlatform;

impl PlatformBackend for WindowsPlatform {
    fn target(&self) -> PlatformTarget {
        target_from_cfg().expect("Windows 目标架构必须受支持")
    }

    fn free_space(&self, path: &Path) -> Result<u64, InstallError> {
        available_space(path).map_err(|error| InstallError::FileSystem(error.to_string()))
    }

    fn has_install_permission(&self, path: &Path) -> Result<bool, InstallError> {
        fs::create_dir_all(path).map_err(|error| InstallError::FileSystem(error.to_string()))?;
        let probe = path.join(format!(".install-permission-{}", std::process::id()));
        match fs::File::create(&probe) {
            Ok(_) => {
                let _ = fs::remove_file(probe);
                Ok(true)
            }
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => Ok(false),
            Err(error) => Err(InstallError::FileSystem(error.to_string())),
        }
    }

    fn create_entry(&self, app_id: AppId, executable: &Path) -> Result<(), InstallError> {
        let app_data = env::var_os("APPDATA")
            .ok_or_else(|| InstallError::EntryCreationFailed("找不到 APPDATA".to_owned()))?;
        let start_menu = PathBuf::from(app_data)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
            .join("Codex Installer");
        fs::create_dir_all(&start_menu)
            .map_err(|error| InstallError::EntryCreationFailed(error.to_string()))?;

        let shortcut = start_menu.join(format!("{app_id}.lnk"));
        let script = "$shell=New-Object -ComObject WScript.Shell;$shortcut=$shell.CreateShortcut($args[0]);$shortcut.TargetPath=$args[1];$shortcut.WorkingDirectory=(Split-Path $args[1]);$shortcut.Save()";
        let status = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                script,
                shortcut.to_string_lossy().as_ref(),
                executable.to_string_lossy().as_ref(),
            ])
            .status()
            .map_err(|error| InstallError::EntryCreationFailed(error.to_string()))?;

        if status.success() {
            Ok(())
        } else {
            Err(InstallError::EntryCreationFailed(format!(
                "PowerShell 返回退出码 {:?}",
                status.code()
            )))
        }
    }

    fn launch(&self, executable: &Path) -> Result<(), InstallError> {
        Command::new(executable)
            .spawn()
            .map(|_| ())
            .map_err(|error| InstallError::LaunchFailed(error.to_string()))
    }

    fn user_applications_dir(&self) -> Result<PathBuf, InstallError> {
        env::var_os("LOCALAPPDATA")
            .map(|value| PathBuf::from(value).join("Programs"))
            .ok_or_else(|| InstallError::FileSystem("找不到 LOCALAPPDATA".to_owned()))
    }
}
