use crate::domain::{AppId, AppRelease, InstallError, InstallerKind, PlatformTarget};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Manifest {
    pub schema_version: u32,
    releases: HashMap<AppId, HashMap<PlatformTarget, AppRelease>>,
}

#[derive(Debug, Deserialize)]
struct RawManifest {
    schema_version: u32,
    apps: HashMap<String, HashMap<String, AppRelease>>,
}

impl Manifest {
    pub fn from_json(input: &str) -> Result<Self, InstallError> {
        let raw: RawManifest = serde_json::from_str(input)
            .map_err(|error| InstallError::InvalidJson(error.to_string()))?;

        if raw.schema_version != 1 {
            return Err(InstallError::InvalidManifest(format!(
                "不支持的 schema_version: {}",
                raw.schema_version
            )));
        }

        let mut releases = HashMap::new();
        for (app_key, targets) in raw.apps {
            let app = match app_key.as_str() {
                "codex" => AppId::Codex,
                "cc-switch" => AppId::CcSwitch,
                _ => {
                    return Err(InstallError::InvalidManifest(format!(
                        "不支持的应用: {app_key}"
                    )))
                }
            };

            let mut parsed_targets = HashMap::new();
            for (target_key, release) in targets {
                let target = PlatformTarget::parse(&target_key)?;
                validate_release(&release)?;
                parsed_targets.insert(target, release);
            }
            releases.insert(app, parsed_targets);
        }

        Ok(Self {
            schema_version: raw.schema_version,
            releases,
        })
    }

    pub fn select(
        &self,
        app_id: AppId,
        target: PlatformTarget,
    ) -> Result<AppRelease, InstallError> {
        self.releases
            .get(&app_id)
            .and_then(|releases| releases.get(&target))
            .cloned()
            .ok_or_else(|| InstallError::ReleaseNotFound {
                app: app_id,
                target: target.key(),
            })
    }
}

fn validate_release(release: &AppRelease) -> Result<(), InstallError> {
    if release.version.trim().is_empty() {
        return Err(InstallError::InvalidManifest("version 不能为空".to_owned()));
    }
    if !release.url.starts_with("https://") {
        return Err(InstallError::InvalidManifest(format!(
            "下载地址必须使用 HTTPS: {}",
            release.url
        )));
    }
    let is_store = release.installer_kind == InstallerKind::WindowsStore;
    if !is_store && release.size_bytes == 0 {
        return Err(InstallError::InvalidManifest(
            "size_bytes 必须大于 0".to_owned(),
        ));
    }
    if !is_store
        && (release.sha256.len() != 64
            || !release
                .sha256
                .chars()
                .all(|character| character.is_ascii_hexdigit()))
    {
        return Err(InstallError::InvalidManifest(
            "sha256 必须是 64 位十六进制字符串".to_owned(),
        ));
    }
    if is_store
        && release
            .store_product_id
            .as_deref()
            .unwrap_or_default()
            .is_empty()
    {
        return Err(InstallError::InvalidManifest(
            "Microsoft Store 安装包必须提供 store_product_id".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Manifest;
    use crate::domain::{AppId, Arch, InstallError, InstallerKind, Os, PlatformTarget};

    const VALID_RELEASE: &str = r#"
    {
      "version": "1.0.0",
      "url": "https://downloads.example.test/codex.exe",
      "size_bytes": 128,
      "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
      "installer_kind": "windows-exe"
    }
    "#;

    fn manifest_with_release(release: &str) -> String {
        format!(r#"{{"schema_version":1,"apps":{{"codex":{{"windows-x64":{release}}}}}}}"#)
    }

    #[test]
    fn selects_release_for_platform() {
        let manifest = Manifest::from_json(&manifest_with_release(VALID_RELEASE)).unwrap();
        let target = PlatformTarget {
            os: Os::Windows,
            arch: Arch::X64,
        };

        assert_eq!(manifest.schema_version, 1);
        assert_eq!(
            manifest.select(AppId::Codex, target).unwrap().version,
            "1.0.0"
        );
    }

    #[test]
    fn rejects_invalid_sha256() {
        let release = VALID_RELEASE.replace(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "bad",
        );

        let error = Manifest::from_json(&manifest_with_release(&release)).unwrap_err();
        assert!(
            matches!(error, InstallError::InvalidManifest(message) if message.contains("sha256"))
        );
    }

    #[test]
    fn rejects_non_https_url() {
        let release = VALID_RELEASE.replace("https://", "http://");
        let error = Manifest::from_json(&manifest_with_release(&release)).unwrap_err();

        assert!(
            matches!(error, InstallError::InvalidManifest(message) if message.contains("HTTPS"))
        );
    }

    #[test]
    fn reports_missing_platform_release() {
        let manifest = Manifest::from_json(&manifest_with_release(VALID_RELEASE)).unwrap();
        let target = PlatformTarget {
            os: Os::Macos,
            arch: Arch::Arm64,
        };

        assert!(matches!(
            manifest.select(AppId::Codex, target),
            Err(InstallError::ReleaseNotFound { .. })
        ));
    }

    #[test]
    fn accepts_microsoft_store_release_without_file_hash() {
        let input = r#"
        {
          "schema_version": 1,
          "apps": {
            "codex": {
              "windows-x64": {
                "version": "store-current",
                "url": "https://apps.microsoft.com/detail/9PLM9XGG6VKS",
                "size_bytes": 0,
                "sha256": "",
                "installer_kind": "windows-store",
                "windows_store_package_name": "OpenAI.Codex",
                "store_product_id": "9PLM9XGG6VKS"
              }
            }
          }
        }
        "#;

        let release = Manifest::from_json(input)
            .unwrap()
            .select(
                AppId::Codex,
                PlatformTarget {
                    os: Os::Windows,
                    arch: Arch::X64,
                },
            )
            .unwrap();

        assert_eq!(release.installer_kind, InstallerKind::WindowsStore);
        assert_eq!(release.store_product_id.as_deref(), Some("9PLM9XGG6VKS"));
    }
}
