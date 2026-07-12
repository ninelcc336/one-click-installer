use serde::{Deserialize, Serialize};
use std::{fmt, path::PathBuf};
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

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct InstalledApp {
    pub app_id: AppId,
    pub version: String,
    pub executable: PathBuf,
}

#[derive(Debug, Clone, Error, Eq, PartialEq)]
pub enum InstallError {
    #[error("网络请求失败: {0}")]
    Network(String),
    #[error("安装包下载失败: {0}")]
    DownloadFailed(String),
    #[error("安装包大小不匹配，需要 {expected} 字节，实际 {actual} 字节")]
    SizeMismatch { expected: u64, actual: u64 },
    #[error("安装包 SHA-256 校验失败")]
    ChecksumMismatch,
    #[error("文件系统操作失败: {0}")]
    FileSystem(String),
    #[error("不支持当前操作系统")]
    UnsupportedPlatform,
    #[error("磁盘空间不足，需要 {required} 字节，可用 {available} 字节")]
    InsufficientDiskSpace { required: u64, available: u64 },
    #[error("没有足够的安装权限: {0}")]
    PermissionDenied(String),
    #[error("启动应用失败: {0}")]
    LaunchFailed(String),
    #[error("创建应用入口失败: {0}")]
    EntryCreationFailed(String),
    #[error("版本清单 JSON 无效: {0}")]
    InvalidJson(String),
    #[error("版本清单无效: {0}")]
    InvalidManifest(String),
    #[error("不支持的平台标识: {0}")]
    InvalidPlatform(String),
    #[error("找不到 {app} 在 {target} 的安装包")]
    ReleaseNotFound { app: AppId, target: String },
}
