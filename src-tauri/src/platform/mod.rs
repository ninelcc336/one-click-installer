#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

use crate::domain::{AppId, Arch, InstallError, Os, PlatformTarget};
use std::path::{Path, PathBuf};

pub trait PlatformBackend: Send + Sync {
    fn target(&self) -> PlatformTarget;
    fn free_space(&self, path: &Path) -> Result<u64, InstallError>;
    fn has_install_permission(&self, path: &Path) -> Result<bool, InstallError>;
    fn create_entry(&self, app_id: AppId, executable: &Path) -> Result<(), InstallError>;
    fn launch(&self, executable: &Path) -> Result<(), InstallError>;
    fn user_applications_dir(&self) -> Result<PathBuf, InstallError>;
}

pub fn current() -> Result<Box<dyn PlatformBackend>, InstallError> {
    #[cfg(target_os = "windows")]
    {
        return Ok(Box::new(windows::WindowsPlatform));
    }

    #[cfg(target_os = "macos")]
    {
        return Ok(Box::new(macos::MacosPlatform));
    }

    #[allow(unreachable_code)]
    Err(InstallError::UnsupportedPlatform)
}

pub fn target_from_cfg() -> Result<PlatformTarget, InstallError> {
    let os = if cfg!(target_os = "windows") {
        Os::Windows
    } else if cfg!(target_os = "macos") {
        Os::Macos
    } else {
        return Err(InstallError::UnsupportedPlatform);
    };

    let arch = if cfg!(target_arch = "x86_64") {
        Arch::X64
    } else if cfg!(target_arch = "aarch64") {
        Arch::Arm64
    } else {
        return Err(InstallError::UnsupportedPlatform);
    };

    Ok(PlatformTarget { os, arch })
}

#[cfg(test)]
mod tests {
    use super::{PlatformBackend, target_from_cfg};
    use crate::domain::{AppId, InstallError, PlatformTarget};
    use std::path::{Path, PathBuf};

    struct FakePlatform;

    impl PlatformBackend for FakePlatform {
        fn target(&self) -> PlatformTarget {
            target_from_cfg().unwrap()
        }

        fn free_space(&self, _path: &Path) -> Result<u64, InstallError> {
            Ok(1024)
        }

        fn has_install_permission(&self, _path: &Path) -> Result<bool, InstallError> {
            Ok(true)
        }

        fn create_entry(&self, _app_id: AppId, _executable: &Path) -> Result<(), InstallError> {
            Ok(())
        }

        fn launch(&self, _executable: &Path) -> Result<(), InstallError> {
            Ok(())
        }

        fn user_applications_dir(&self) -> Result<PathBuf, InstallError> {
            Ok(PathBuf::from("C:/Users/test/Applications"))
        }
    }

    #[test]
    fn fake_platform_exposes_current_target_and_capabilities() {
        let platform = FakePlatform;

        assert!(platform.target().key().contains('-'));
        assert_eq!(platform.free_space(Path::new(".")).unwrap(), 1024);
        assert!(platform.has_install_permission(Path::new(".")).unwrap());
        platform
            .create_entry(AppId::Codex, Path::new("codex.exe"))
            .unwrap();
        platform.launch(Path::new("codex.exe")).unwrap();
    }
}
