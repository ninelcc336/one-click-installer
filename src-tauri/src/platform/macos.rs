use super::{target_from_cfg, PlatformBackend};
use crate::domain::{AppId, InstallError, PlatformTarget};
use fs2::available_space;
use std::{env, fs, path::{Path, PathBuf}, process::Command};

pub struct MacosPlatform;

impl PlatformBackend for MacosPlatform {
    fn target(&self) -> PlatformTarget {
        target_from_cfg().expect("macOS 目标架构必须受支持")
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

    fn create_entry(&self, _app_id: AppId, _executable: &Path) -> Result<(), InstallError> {
        Ok(())
    }

    fn launch(&self, executable: &Path) -> Result<(), InstallError> {
        Command::new("open")
            .arg(executable)
            .spawn()
            .map(|_| ())
            .map_err(|error| InstallError::LaunchFailed(error.to_string()))
    }

    fn user_applications_dir(&self) -> Result<PathBuf, InstallError> {
        env::var_os("HOME")
            .map(|value| PathBuf::from(value).join("Applications"))
            .ok_or_else(|| InstallError::FileSystem("找不到 HOME".to_owned()))
    }
}
