use crate::{
    domain::{AppId, AppRelease, InstallError, InstalledApp, InstallerKind, Os, PlatformTarget},
    platform::PlatformBackend,
};
#[cfg(target_os = "macos")]
use std::fs;
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

pub mod cc_switch;
pub mod codex;

pub trait AppAdapter: Send + Sync {
    fn id(&self) -> AppId;
    fn candidate_paths(
        &self,
        release: &AppRelease,
        target: PlatformTarget,
        install_dir: &Path,
    ) -> Vec<PathBuf>;
    fn detect(
        &self,
        release: &AppRelease,
        platform: &dyn PlatformBackend,
        install_dir: &Path,
    ) -> Result<Option<InstalledApp>, InstallError>;
    fn install(
        &self,
        release: &AppRelease,
        package: &Path,
        platform: &dyn PlatformBackend,
        install_dir: &Path,
    ) -> Result<(), InstallError>;
    fn verify(
        &self,
        release: &AppRelease,
        platform: &dyn PlatformBackend,
        install_dir: &Path,
    ) -> Result<InstalledApp, InstallError>;
    fn launch(
        &self,
        app: &InstalledApp,
        platform: &dyn PlatformBackend,
    ) -> Result<(), InstallError>;
}

#[derive(Debug, Clone, Copy)]
pub struct DesktopAppAdapter {
    pub app_id: AppId,
}

impl DesktopAppAdapter {
    pub fn new(app_id: AppId) -> Self {
        Self { app_id }
    }
}

impl AppAdapter for DesktopAppAdapter {
    fn id(&self) -> AppId {
        self.app_id
    }

    fn candidate_paths(
        &self,
        release: &AppRelease,
        target: PlatformTarget,
        install_dir: &Path,
    ) -> Vec<PathBuf> {
        match target.os {
            Os::Windows => release
                .windows_executable
                .as_deref()
                .map(|path| expand_windows_paths(path, install_dir))
                .unwrap_or_default(),
            Os::Macos => release
                .macos_bundle
                .as_deref()
                .map(|bundle| expand_macos_paths(bundle, install_dir))
                .unwrap_or_default(),
        }
    }

    fn detect(
        &self,
        release: &AppRelease,
        platform: &dyn PlatformBackend,
        install_dir: &Path,
    ) -> Result<Option<InstalledApp>, InstallError> {
        let target = platform.target();
        if target.os == Os::Windows && release.installer_kind == InstallerKind::WindowsStore {
            return detect_windows_store(self.app_id, release);
        }
        Ok(self
            .candidate_paths(release, target, install_dir)
            .into_iter()
            .find(|path| path.exists())
            .map(|executable| InstalledApp {
                app_id: self.app_id,
                version: "unknown".to_owned(),
                executable,
            }))
    }

    fn install(
        &self,
        release: &AppRelease,
        package: &Path,
        platform: &dyn PlatformBackend,
        install_dir: &Path,
    ) -> Result<(), InstallError> {
        match (platform.target().os, release.installer_kind) {
            (Os::Windows, InstallerKind::WindowsExe) => run_command(package, &[]),
            (Os::Windows, InstallerKind::WindowsMsi) => {
                let install_dir_arg = format!("INSTALLDIR={}", install_dir.display());
                run_command(
                    Path::new("msiexec.exe"),
                    &["/i", package.to_string_lossy().as_ref(), &install_dir_arg],
                )
            }
            (Os::Windows, InstallerKind::WindowsStore) => {
                let product_id = release.store_product_id.as_deref().ok_or_else(|| {
                    InstallError::InstallerFailed("缺少 Microsoft Store 产品 ID".to_owned())
                })?;
                run_command(
                    Path::new("winget.exe"),
                    &[
                        "install",
                        "--id",
                        product_id,
                        "--exact",
                        "--source",
                        "msstore",
                        "--accept-source-agreements",
                        "--accept-package-agreements",
                    ],
                )
            }
            (Os::Macos, InstallerKind::MacosPkg) => run_command(
                Path::new("/usr/sbin/installer"),
                &[
                    "-pkg",
                    package.to_string_lossy().as_ref(),
                    "-target",
                    "CurrentUserHomeDirectory",
                ],
            ),
            (Os::Macos, InstallerKind::MacosDmg) => install_macos_dmg(package, install_dir),
            _ => Err(InstallError::InstallerFailed(format!(
                "安装包类型 {:?} 与当前平台不匹配",
                release.installer_kind
            ))),
        }
    }

    fn verify(
        &self,
        release: &AppRelease,
        platform: &dyn PlatformBackend,
        install_dir: &Path,
    ) -> Result<InstalledApp, InstallError> {
        self.detect(release, platform, install_dir)?.ok_or_else(|| {
            InstallError::VerificationFailed(format!("找不到 {} 的安装结果", self.app_id))
        })
    }

    fn launch(
        &self,
        app: &InstalledApp,
        platform: &dyn PlatformBackend,
    ) -> Result<(), InstallError> {
        platform.launch(&app.executable)
    }
}

fn run_command(program: &Path, args: &[&str]) -> Result<(), InstallError> {
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|error| InstallError::InstallerFailed(error.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(InstallError::InstallerFailed(format!(
            "{} 返回退出码 {:?}",
            program.display(),
            status.code()
        )))
    }
}

#[cfg(target_os = "windows")]
fn detect_windows_store(
    app_id: AppId,
    release: &AppRelease,
) -> Result<Option<InstalledApp>, InstallError> {
    let package_name = release
        .windows_store_package_name
        .as_deref()
        .ok_or_else(|| InstallError::VerificationFailed(format!("{} 缺少 Store 包名", app_id)))?;
    let package_literal = package_name.replace('\'', "''");
    let script = format!(
        "$package=Get-AppxPackage -Name '{}' | Select-Object -First 1; if ($package) {{ Write-Output $package.PackageFamilyName }}",
        package_literal
    );
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(|error| InstallError::VerificationFailed(error.to_string()))?;
    if !output.status.success() {
        return Ok(None);
    }
    let family_name = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if family_name.is_empty() {
        return Ok(None);
    }
    Ok(Some(InstalledApp {
        app_id,
        version: "store".to_owned(),
        executable: PathBuf::from(format!("shell:AppsFolder\\{family_name}!App")),
    }))
}

#[cfg(not(target_os = "windows"))]
fn detect_windows_store(
    _app_id: AppId,
    _release: &AppRelease,
) -> Result<Option<InstalledApp>, InstallError> {
    Ok(None)
}

fn expand_windows_paths(relative_or_absolute: &str, install_dir: &Path) -> Vec<PathBuf> {
    let path = PathBuf::from(relative_or_absolute);
    if path.is_absolute() {
        return vec![path];
    }

    let mut relative_paths = vec![path.clone()];
    if path
        .components()
        .next()
        .is_some_and(|component| component.as_os_str().eq_ignore_ascii_case("Programs"))
    {
        relative_paths.push(path.iter().skip(1).collect());
    }

    let mut paths = Vec::new();
    for relative_path in &relative_paths {
        let install_dir_is_programs = install_dir
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("Programs"));
        let relative_path_starts_with_programs = relative_path
            .components()
            .next()
            .is_some_and(|component| component.as_os_str().eq_ignore_ascii_case("Programs"));
        if !(install_dir_is_programs && relative_path_starts_with_programs) {
            paths.push(install_dir.join(relative_path));
        }
        paths.extend(
            [
                "LOCALAPPDATA",
                "PROGRAMFILES",
                "PROGRAMFILES(X86)",
                "USERPROFILE",
            ]
            .into_iter()
            .filter_map(|name| env::var_os(name).map(PathBuf::from))
            .map(|root| root.join(relative_path)),
        );
    }

    paths.sort();
    paths.dedup();
    paths
}

fn expand_macos_paths(bundle: &str, install_dir: &Path) -> Vec<PathBuf> {
    let bundle_path = PathBuf::from(bundle);
    if bundle_path.is_absolute() {
        return vec![bundle_path];
    }

    let mut paths = vec![
        install_dir.join(&bundle_path),
        PathBuf::from("/Applications").join(&bundle_path),
    ];
    if let Some(home) = env::var_os("HOME") {
        paths.push(PathBuf::from(home).join("Applications").join(bundle_path));
    }
    paths
}

#[cfg(target_os = "macos")]
fn install_macos_dmg(package: &Path, install_dir: &Path) -> Result<(), InstallError> {
    let output = Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-readonly", "-plist"])
        .arg(package)
        .output()
        .map_err(|error| InstallError::InstallerFailed(error.to_string()))?;
    if !output.status.success() {
        return Err(InstallError::InstallerFailed("无法挂载 DMG".to_owned()));
    }

    let mount_point = String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.trim().strip_prefix("<string>/Volumes/"))
        .map(|value| PathBuf::from("/Volumes").join(value.trim_end_matches("</string>")))
        .ok_or_else(|| InstallError::InstallerFailed("无法解析 DMG 挂载点".to_owned()))?;
    let result = copy_first_app_bundle(&mount_point, install_dir);
    let _ = Command::new("hdiutil")
        .args(["detach"])
        .arg(&mount_point)
        .status();
    result
}

#[cfg(not(target_os = "macos"))]
fn install_macos_dmg(_package: &Path, _install_dir: &Path) -> Result<(), InstallError> {
    Err(InstallError::UnsupportedPlatform)
}

#[cfg(target_os = "macos")]
fn copy_first_app_bundle(mount_point: &Path, destination: &Path) -> Result<(), InstallError> {
    let bundle = fs::read_dir(mount_point)
        .map_err(|error| InstallError::InstallerFailed(error.to_string()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|extension| extension == "app"))
        .ok_or_else(|| InstallError::InstallerFailed("DMG 中没有找到应用".to_owned()))?;
    fs::create_dir_all(destination)
        .map_err(|error| InstallError::InstallerFailed(error.to_string()))?;
    let target = destination.join(bundle.file_name().unwrap_or_default());
    if target.exists() {
        fs::remove_dir_all(&target)
            .map_err(|error| InstallError::InstallerFailed(error.to_string()))?;
    }
    copy_dir_all(&bundle, &target)
}

#[cfg(target_os = "macos")]
fn copy_dir_all(source: &Path, destination: &Path) -> Result<(), InstallError> {
    fs::create_dir_all(destination)
        .map_err(|error| InstallError::InstallerFailed(error.to_string()))?;
    for entry in
        fs::read_dir(source).map_err(|error| InstallError::InstallerFailed(error.to_string()))?
    {
        let entry = entry.map_err(|error| InstallError::InstallerFailed(error.to_string()))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_all(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path)
                .map_err(|error| InstallError::InstallerFailed(error.to_string()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{expand_macos_paths, expand_windows_paths, DesktopAppAdapter};
    use crate::domain::{AppId, AppRelease, InstallerKind};
    use std::path::Path;

    fn release() -> AppRelease {
        AppRelease {
            version: "1.0.0".to_owned(),
            url: "https://example.test/package".to_owned(),
            size_bytes: 1,
            sha256: "0".repeat(64),
            installer_kind: InstallerKind::WindowsExe,
            windows_executable: Some("Codex/Codex.exe".to_owned()),
            windows_store_package_name: None,
            store_product_id: None,
            macos_bundle: Some("Codex.app".to_owned()),
        }
    }

    #[test]
    fn expands_relative_windows_path_under_known_roots() {
        let paths = expand_windows_paths("Codex/Codex.exe", Path::new("C:/Install"));
        assert!(!paths.is_empty());
        assert!(paths.iter().any(|path| path.ends_with("Codex/Codex.exe")));
    }

    #[test]
    fn expands_cc_switch_path_under_user_programs_directory() {
        let paths = expand_windows_paths(
            "Programs/CC Switch/cc-switch.exe",
            Path::new("C:/Users/test/AppData/Local/Programs"),
        );
        assert!(paths
            .iter()
            .any(|path| { path.ends_with("AppData/Local/Programs/CC Switch/cc-switch.exe") }));
        assert!(!paths.iter().any(|path| {
            path.ends_with("AppData/Local/Programs/Programs/CC Switch/cc-switch.exe")
        }));
    }

    #[test]
    fn expands_macos_bundle_under_application_directories() {
        let paths = expand_macos_paths("Codex.app", Path::new("C:/Install"));
        assert!(paths
            .iter()
            .any(|path| path.ends_with("Applications/Codex.app")));
    }

    #[test]
    fn adapters_keep_app_identity_separate() {
        assert_eq!(DesktopAppAdapter::new(AppId::Codex).app_id, AppId::Codex);
        assert_eq!(
            DesktopAppAdapter::new(AppId::CcSwitch).app_id,
            AppId::CcSwitch
        );
        assert_eq!(release().version, "1.0.0");
    }
}
