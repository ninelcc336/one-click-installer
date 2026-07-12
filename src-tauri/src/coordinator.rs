use crate::{
    adapters::AppAdapter,
    domain::{AppId, AppRelease, InstallError, InstalledApp, InstallerKind, PlatformTarget},
    download::{Downloader, ProgressSink},
    manifest::Manifest,
    platform::PlatformBackend,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum InstallPhase {
    Checking,
    Planning,
    Downloading,
    VerifyingPackage,
    Installing,
    VerifyingInstall,
    CreatingEntry,
    Skipped,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct InstallEvent {
    pub app_id: Option<AppId>,
    pub phase: InstallPhase,
    pub downloaded: Option<u64>,
    pub total: Option<u64>,
    pub message_key: String,
}

pub trait EventSink: Send + Sync {
    fn emit(&self, event: InstallEvent);
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct InstallCheckpoint {
    pub current_app: Option<AppId>,
    pub phase: InstallPhase,
    pub completed: Vec<AppId>,
}

#[derive(Debug, Clone)]
pub struct InstallItem {
    pub app_id: AppId,
    pub release: AppRelease,
    pub package_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct InstallPlan {
    pub target: PlatformTarget,
    pub items: Vec<InstallItem>,
}

#[derive(Debug, Clone, Default)]
pub struct InstallSummary {
    pub installed: Vec<InstalledApp>,
    pub skipped: Vec<AppId>,
}

pub struct InstallCoordinator {
    manifest: Manifest,
    platform: Box<dyn PlatformBackend>,
    install_dir: PathBuf,
    adapters: Vec<Box<dyn AppAdapter>>,
    downloader: Box<dyn Downloader>,
    checkpoint_path: PathBuf,
}

impl InstallCoordinator {
    pub fn new(
        manifest: Manifest,
        platform: Box<dyn PlatformBackend>,
        install_dir: PathBuf,
        adapters: Vec<Box<dyn AppAdapter>>,
        downloader: Box<dyn Downloader>,
        checkpoint_path: PathBuf,
    ) -> Self {
        Self {
            manifest,
            platform,
            install_dir,
            adapters,
            downloader,
            checkpoint_path,
        }
    }

    pub fn plan(
        &self,
        cache_dir: &Path,
        selected_apps: &[AppId],
    ) -> Result<InstallPlan, InstallError> {
        let target = self.platform.target();
        let mut items = Vec::new();
        for app_id in selected_apps.iter().copied() {
            let release = self.manifest.select(app_id, target)?;
            let package_path =
                cache_dir.join(format!("{}-{}", app_id.as_manifest_key(), release.version));
            items.push(InstallItem {
                app_id,
                release,
                package_path,
            });
        }
        Ok(InstallPlan { target, items })
    }

    pub fn run(
        &self,
        plan: &InstallPlan,
        sink: &dyn EventSink,
    ) -> Result<InstallSummary, InstallError> {
        let mut summary = InstallSummary::default();
        let checkpoint = self.resume().unwrap_or(InstallCheckpoint {
            current_app: None,
            phase: InstallPhase::Planning,
            completed: Vec::new(),
        });

        for item in &plan.items {
            if checkpoint.completed.contains(&item.app_id) {
                summary.skipped.push(item.app_id);
                continue;
            }

            let adapter = self
                .adapters
                .iter()
                .find(|adapter| adapter.id() == item.app_id)
                .ok_or_else(|| {
                    InstallError::VerificationFailed(format!("缺少 {} 适配器", item.app_id))
                })?;

            if let Some(existing) =
                adapter.detect(&item.release, self.platform.as_ref(), &self.install_dir)?
            {
                self.emit_and_save(
                    sink,
                    InstallEvent {
                        app_id: Some(item.app_id),
                        phase: InstallPhase::Skipped,
                        downloaded: None,
                        total: None,
                        message_key: "app.already-installed".to_owned(),
                    },
                    &InstallCheckpoint {
                        current_app: Some(item.app_id),
                        phase: InstallPhase::Skipped,
                        completed: vec![item.app_id],
                    },
                )?;
                summary.skipped.push(item.app_id);
                let _ = existing;
                continue;
            }

            let package = if item.release.installer_kind == InstallerKind::WindowsStore {
                item.package_path.clone()
            } else {
                self.emit_and_save(
                    sink,
                    event(item.app_id, InstallPhase::Downloading, "download.started"),
                    &InstallCheckpoint {
                        current_app: Some(item.app_id),
                        phase: InstallPhase::Downloading,
                        completed: summary.skipped.clone(),
                    },
                )?;
                let progress = CoordinatorProgress { sink };
                match self.downloader.download(
                    item.app_id,
                    &item.release,
                    &item.package_path,
                    &progress,
                ) {
                    Ok(package) => package,
                    Err(error) => {
                        self.save_checkpoint(&InstallCheckpoint {
                            current_app: Some(item.app_id),
                            phase: InstallPhase::Failed,
                            completed: summary.skipped.clone(),
                        })?;
                        sink.emit(event(item.app_id, InstallPhase::Failed, "download.failed"));
                        return Err(error);
                    }
                }
            };

            self.emit_and_save(
                sink,
                event(item.app_id, InstallPhase::Installing, "install.started"),
                &InstallCheckpoint {
                    current_app: Some(item.app_id),
                    phase: InstallPhase::Installing,
                    completed: summary.skipped.clone(),
                },
            )?;
            if let Err(error) = adapter.install(
                &item.release,
                &package,
                self.platform.as_ref(),
                &self.install_dir,
            ) {
                self.save_checkpoint(&InstallCheckpoint {
                    current_app: Some(item.app_id),
                    phase: InstallPhase::Failed,
                    completed: summary.skipped.clone(),
                })?;
                sink.emit(event(item.app_id, InstallPhase::Failed, "install.failed"));
                return Err(error);
            }

            self.emit_and_save(
                sink,
                event(
                    item.app_id,
                    InstallPhase::VerifyingInstall,
                    "verify.started",
                ),
                &InstallCheckpoint {
                    current_app: Some(item.app_id),
                    phase: InstallPhase::VerifyingInstall,
                    completed: summary.skipped.clone(),
                },
            )?;
            let installed =
                match adapter.verify(&item.release, self.platform.as_ref(), &self.install_dir) {
                    Ok(installed) => installed,
                    Err(error) => {
                        self.save_checkpoint(&InstallCheckpoint {
                            current_app: Some(item.app_id),
                            phase: InstallPhase::Failed,
                            completed: summary.skipped.clone(),
                        })?;
                        sink.emit(event(item.app_id, InstallPhase::Failed, "verify.failed"));
                        return Err(error);
                    }
                };

            self.emit_and_save(
                sink,
                event(item.app_id, InstallPhase::CreatingEntry, "entry.started"),
                &InstallCheckpoint {
                    current_app: Some(item.app_id),
                    phase: InstallPhase::CreatingEntry,
                    completed: summary.skipped.clone(),
                },
            )?;
            self.platform
                .create_entry(item.app_id, &installed.executable)?;
            summary.installed.push(installed);

            let mut completed = summary.skipped.clone();
            completed.extend(summary.installed.iter().map(|app| app.app_id));
            self.emit_and_save(
                sink,
                event(item.app_id, InstallPhase::Completed, "app.completed"),
                &InstallCheckpoint {
                    current_app: Some(item.app_id),
                    phase: InstallPhase::Completed,
                    completed,
                },
            )?;
        }

        self.save_checkpoint(&InstallCheckpoint {
            current_app: None,
            phase: InstallPhase::Completed,
            completed: plan.items.iter().map(|item| item.app_id).collect(),
        })?;
        Ok(summary)
    }

    pub fn resume(&self) -> Option<InstallCheckpoint> {
        let data = fs::read_to_string(&self.checkpoint_path).ok()?;
        serde_json::from_str(&data).ok()
    }

    fn emit_and_save(
        &self,
        sink: &dyn EventSink,
        event: InstallEvent,
        checkpoint: &InstallCheckpoint,
    ) -> Result<(), InstallError> {
        sink.emit(event);
        self.save_checkpoint(checkpoint)
    }

    fn save_checkpoint(&self, checkpoint: &InstallCheckpoint) -> Result<(), InstallError> {
        if let Some(parent) = self.checkpoint_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| InstallError::FileSystem(error.to_string()))?;
        }
        let temporary = self.checkpoint_path.with_extension("tmp");
        let data = serde_json::to_vec_pretty(checkpoint)
            .map_err(|error| InstallError::FileSystem(error.to_string()))?;
        fs::write(&temporary, data).map_err(|error| InstallError::FileSystem(error.to_string()))?;
        if self.checkpoint_path.exists() {
            fs::remove_file(&self.checkpoint_path)
                .map_err(|error| InstallError::FileSystem(error.to_string()))?;
        }
        fs::rename(temporary, &self.checkpoint_path)
            .map_err(|error| InstallError::FileSystem(error.to_string()))
    }
}

struct CoordinatorProgress<'a> {
    sink: &'a dyn EventSink,
}

impl ProgressSink for CoordinatorProgress<'_> {
    fn on_progress(&self, app_id: AppId, downloaded: u64, total: u64) {
        self.sink.emit(InstallEvent {
            app_id: Some(app_id),
            phase: InstallPhase::Downloading,
            downloaded: Some(downloaded),
            total: Some(total),
            message_key: "download.progress".to_owned(),
        });
    }
}

fn event(app_id: AppId, phase: InstallPhase, message_key: &str) -> InstallEvent {
    InstallEvent {
        app_id: Some(app_id),
        phase,
        downloaded: None,
        total: None,
        message_key: message_key.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::{AppRelease, Os},
        platform::PlatformBackend,
    };
    use std::{
        path::Path,
        sync::{Arc, Mutex},
    };
    use tempfile::tempdir;

    struct FakePlatform;

    impl PlatformBackend for FakePlatform {
        fn target(&self) -> PlatformTarget {
            PlatformTarget {
                os: Os::Windows,
                arch: crate::domain::Arch::X64,
            }
        }
        fn free_space(&self, _path: &Path) -> Result<u64, InstallError> {
            Ok(u64::MAX)
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
            Ok(PathBuf::from("C:/Applications"))
        }
    }

    struct FakeAdapter {
        app_id: AppId,
        root: PathBuf,
    }

    impl AppAdapter for FakeAdapter {
        fn id(&self) -> AppId {
            self.app_id
        }
        fn candidate_paths(
            &self,
            _release: &AppRelease,
            _target: PlatformTarget,
            _install_dir: &Path,
        ) -> Vec<PathBuf> {
            vec![self
                .root
                .join(format!("{}.exe", self.app_id.as_manifest_key()))]
        }
        fn detect(
            &self,
            release: &AppRelease,
            platform: &dyn PlatformBackend,
            install_dir: &Path,
        ) -> Result<Option<InstalledApp>, InstallError> {
            Ok(self
                .candidate_paths(release, platform.target(), install_dir)
                .into_iter()
                .find(|path| path.exists())
                .map(|executable| InstalledApp {
                    app_id: self.app_id,
                    version: "1.0.0".to_owned(),
                    executable,
                }))
        }
        fn install(
            &self,
            _release: &AppRelease,
            _package: &Path,
            _platform: &dyn PlatformBackend,
            _install_dir: &Path,
        ) -> Result<(), InstallError> {
            fs::create_dir_all(&self.root).unwrap();
            fs::write(
                self.root
                    .join(format!("{}.exe", self.app_id.as_manifest_key())),
                b"installed",
            )
            .unwrap();
            Ok(())
        }
        fn verify(
            &self,
            release: &AppRelease,
            platform: &dyn PlatformBackend,
            install_dir: &Path,
        ) -> Result<InstalledApp, InstallError> {
            self.detect(release, platform, install_dir)?
                .ok_or_else(|| InstallError::VerificationFailed("fake verify failed".to_owned()))
        }
        fn launch(
            &self,
            _app: &InstalledApp,
            _platform: &dyn PlatformBackend,
        ) -> Result<(), InstallError> {
            Ok(())
        }
    }

    struct FakeDownloader;

    impl Downloader for FakeDownloader {
        fn download(
            &self,
            app_id: AppId,
            _release: &AppRelease,
            destination: &Path,
            sink: &dyn ProgressSink,
        ) -> Result<PathBuf, InstallError> {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(destination, app_id.as_manifest_key().as_bytes()).unwrap();
            sink.on_progress(app_id, 1, 1);
            Ok(destination.to_path_buf())
        }
    }

    struct RecordingSink(Arc<Mutex<Vec<InstallEvent>>>);

    impl EventSink for RecordingSink {
        fn emit(&self, event: InstallEvent) {
            self.0.lock().unwrap().push(event);
        }
    }

    fn manifest() -> Manifest {
        let release = |name: &str| {
            format!(
                r#"{{"version":"1.0.0","url":"https://example.test/{name}","size_bytes":1,"sha256":"{}","installer_kind":"windows-exe"}}"#,
                "0".repeat(64)
            )
        };
        Manifest::from_json(&format!(r#"{{"schema_version":1,"apps":{{"codex":{{"windows-x64":{}}},"cc-switch":{{"windows-x64":{}}}}}}}"#, release("codex"), release("cc-switch"))).unwrap()
    }

    #[test]
    fn runs_both_apps_and_persists_completion() {
        let directory = tempdir().unwrap();
        let recorder = RecordingSink(Arc::new(Mutex::new(Vec::new())));
        let coordinator = InstallCoordinator::new(
            manifest(),
            Box::new(FakePlatform),
            directory.path().join("install"),
            vec![
                Box::new(FakeAdapter {
                    app_id: AppId::Codex,
                    root: directory.path().join("codex"),
                }),
                Box::new(FakeAdapter {
                    app_id: AppId::CcSwitch,
                    root: directory.path().join("cc-switch"),
                }),
            ],
            Box::new(FakeDownloader),
            directory.path().join("checkpoint.json"),
        );

        let plan = coordinator
            .plan(
                &directory.path().join("cache"),
                &[AppId::Codex, AppId::CcSwitch],
            )
            .unwrap();
        let summary = coordinator.run(&plan, &recorder).unwrap();

        assert_eq!(summary.installed.len(), 2);
        assert_eq!(coordinator.resume().unwrap().phase, InstallPhase::Completed);
        assert!(recorder
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|event| event.phase == InstallPhase::Completed));
    }
}
