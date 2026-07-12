use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AppId {
    Codex,
    CcSwitch,
}

impl AppId {
    pub fn as_manifest_key(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::CcSwitch => "cc-switch",
        }
    }
}

impl fmt::Display for AppId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Codex => "Codex",
            Self::CcSwitch => "CC Switch",
        })
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Os {
    Windows,
    Macos,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Arch {
    X64,
    Arm64,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct PlatformTarget {
    pub os: Os,
    pub arch: Arch,
}

impl PlatformTarget {
    pub fn key(self) -> String {
        format!("{}-{}", self.os, self.arch)
    }

    pub fn parse(value: &str) -> Result<Self, InstallError> {
        let (os, arch) = value
            .split_once('-')
            .ok_or_else(|| InstallError::InvalidPlatform(value.to_owned()))?;

        let os = match os {
            "windows" => Os::Windows,
            "macos" => Os::Macos,
            _ => return Err(InstallError::InvalidPlatform(value.to_owned())),
        };
        let arch = match arch {
            "x64" => Arch::X64,
            "arm64" => Arch::Arm64,
            _ => return Err(InstallError::InvalidPlatform(value.to_owned())),
        };

        Ok(Self { os, arch })
    }
}

impl fmt::Display for Os {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Windows => "windows",
            Self::Macos => "macos",
        })
    }
}

impl fmt::Display for Arch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::X64 => "x64",
            Self::Arm64 => "arm64",
        })
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallerKind {
    WindowsExe,
    WindowsMsi,
    MacosDmg,
    MacosPkg,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppRelease {
    pub version: String,
    pub url: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub installer_kind: InstallerKind,
}

#[derive(Debug, Clone, Error, Eq, PartialEq)]
pub enum InstallError {
    #[error("版本清单 JSON 无效: {0}")]
    InvalidJson(String),
    #[error("版本清单无效: {0}")]
    InvalidManifest(String),
    #[error("不支持的平台标识: {0}")]
    InvalidPlatform(String),
    #[error("找不到 {app} 在 {target} 的安装包")]
    ReleaseNotFound { app: AppId, target: String },
}
