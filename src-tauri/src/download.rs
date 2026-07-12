use crate::domain::{AppId, AppRelease, InstallError};
use reqwest::blocking::Client;
use reqwest::header::RANGE;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

pub trait ProgressSink: Send + Sync {
    fn on_progress(&self, app_id: AppId, downloaded: u64, total: u64);
}

pub trait Downloader: Send + Sync {
    fn download(
        &self,
        app_id: AppId,
        release: &AppRelease,
        destination: &Path,
        sink: &dyn ProgressSink,
    ) -> Result<PathBuf, InstallError>;
}

pub struct DownloadManager {
    client: Client,
    max_retries: u8,
}

impl DownloadManager {
    pub fn new(max_retries: u8) -> Result<Self, InstallError> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|error| InstallError::Network(error.to_string()))?;
        Ok(Self {
            client,
            max_retries,
        })
    }

    pub fn fetch(
        &self,
        app_id: AppId,
        release: &AppRelease,
        destination: &Path,
        sink: &dyn ProgressSink,
    ) -> Result<PathBuf, InstallError> {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| InstallError::DownloadFailed(error.to_string()))?;
        }

        let partial = partial_path(destination);
        let mut last_error = None;

        for _attempt in 0..=self.max_retries {
            match self.fetch_once(app_id, release, destination, &partial, sink) {
                Ok(path) => return Ok(path),
                Err(error @ InstallError::ChecksumMismatch)
                | Err(error @ InstallError::SizeMismatch { .. }) => {
                    let _ = fs::remove_file(&partial);
                    last_error = Some(error);
                }
                Err(error) => last_error = Some(error),
            }
        }

        Err(last_error.unwrap_or_else(|| InstallError::DownloadFailed("下载未执行".to_owned())))
    }

    fn fetch_once(
        &self,
        app_id: AppId,
        release: &AppRelease,
        destination: &Path,
        partial: &Path,
        sink: &dyn ProgressSink,
    ) -> Result<PathBuf, InstallError> {
        let existing = fs::metadata(partial)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        let mut request = self.client.get(&release.url);
        if existing > 0 {
            request = request.header(RANGE, format!("bytes={existing}-"));
        }

        let mut response = request
            .send()
            .map_err(|error| InstallError::Network(error.to_string()))?;
        let requested_resume = existing > 0;
        let append = requested_resume && response.status() == reqwest::StatusCode::PARTIAL_CONTENT;

        if !response.status().is_success() {
            return Err(InstallError::DownloadFailed(format!(
                "服务器返回 HTTP {}",
                response.status()
            )));
        }

        let offset = if append { existing } else { 0 };
        let mut file = if append {
            OpenOptions::new()
                .append(true)
                .open(partial)
                .map_err(|error| InstallError::DownloadFailed(error.to_string()))?
        } else {
            File::create(partial)
                .map_err(|error| InstallError::DownloadFailed(error.to_string()))?
        };
        let total = response
            .content_length()
            .unwrap_or(0)
            .saturating_add(offset);
        let mut downloaded = offset;
        let mut buffer = [0_u8; 64 * 1024];

        sink.on_progress(app_id, downloaded, total);
        loop {
            let read = response
                .read(&mut buffer)
                .map_err(|error| InstallError::DownloadFailed(error.to_string()))?;
            if read == 0 {
                break;
            }
            file.write_all(&buffer[..read])
                .map_err(|error| InstallError::DownloadFailed(error.to_string()))?;
            downloaded = downloaded.saturating_add(read as u64);
            sink.on_progress(app_id, downloaded, total);
        }
        file.flush()
            .map_err(|error| InstallError::DownloadFailed(error.to_string()))?;
        drop(file);

        let actual_size = fs::metadata(partial)
            .map_err(|error| InstallError::DownloadFailed(error.to_string()))?
            .len();
        if actual_size != release.size_bytes {
            return Err(InstallError::SizeMismatch {
                expected: release.size_bytes,
                actual: actual_size,
            });
        }
        if sha256_file(partial)? != release.sha256.to_ascii_lowercase() {
            return Err(InstallError::ChecksumMismatch);
        }

        if destination.exists() {
            fs::remove_file(destination)
                .map_err(|error| InstallError::DownloadFailed(error.to_string()))?;
        }
        fs::rename(partial, destination)
            .map_err(|error| InstallError::DownloadFailed(error.to_string()))?;
        Ok(destination.to_path_buf())
    }
}

impl Downloader for DownloadManager {
    fn download(
        &self,
        app_id: AppId,
        release: &AppRelease,
        destination: &Path,
        sink: &dyn ProgressSink,
    ) -> Result<PathBuf, InstallError> {
        self.fetch(app_id, release, destination, sink)
    }
}

pub fn sha256_file(path: &Path) -> Result<String, InstallError> {
    let mut file =
        File::open(path).map_err(|error| InstallError::DownloadFailed(error.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| InstallError::DownloadFailed(error.to_string()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn partial_path(destination: &Path) -> PathBuf {
    let mut value = destination.as_os_str().to_os_string();
    value.push(".part");
    PathBuf::from(value)
}

#[cfg(test)]
mod tests {
    use super::{sha256_file, DownloadManager, ProgressSink};
    use crate::domain::{AppId, AppRelease, InstallError, InstallerKind};
    use sha2::{Digest, Sha256};
    use std::{
        io::{Read, Write},
        net::TcpListener,
        path::Path,
        sync::{Arc, Mutex},
        thread,
    };
    use tempfile::tempdir;

    struct RecordingSink(Arc<Mutex<Vec<u64>>>);

    impl ProgressSink for RecordingSink {
        fn on_progress(&self, _app_id: AppId, downloaded: u64, _total: u64) {
            self.0.lock().unwrap().push(downloaded);
        }
    }

    fn release_for(body: &[u8], sha256: String, url: String) -> AppRelease {
        AppRelease {
            version: "1.0.0".to_owned(),
            url,
            size_bytes: body.len() as u64,
            sha256,
            installer_kind: InstallerKind::WindowsExe,
            windows_executable: None,
            macos_bundle: None,
        }
    }

    fn sha256_bytes(body: &[u8]) -> String {
        hex::encode(Sha256::digest(body))
    }

    fn serve_once(listener: TcpListener, body: Vec<u8>, status: &str) {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 1024];
        let _ = stream.read(&mut request);
        let header = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(header.as_bytes()).unwrap();
        stream.write_all(&body).unwrap();
    }

    #[test]
    fn downloads_and_verifies_file() {
        let body = b"test installer bytes".to_vec();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/package.exe", listener.local_addr().unwrap());
        let server = thread::spawn({
            let body = body.clone();
            move || serve_once(listener, body, "200 OK")
        });

        let directory = tempdir().unwrap();
        let destination = directory.path().join("package.exe");
        let sink = RecordingSink(Arc::new(Mutex::new(Vec::new())));
        let release = release_for(&body, sha256_bytes(&body), url);
        let manager = DownloadManager::new(0).unwrap();

        let output = manager
            .fetch(AppId::Codex, &release, &destination, &sink)
            .unwrap();

        server.join().unwrap();
        assert_eq!(output, destination);
        assert_eq!(std::fs::read(output).unwrap(), body);
        assert!(!destination.with_extension("exe.part").exists());
        assert!(!sink.0.lock().unwrap().is_empty());
    }

    #[test]
    fn rejects_checksum_and_removes_partial_file() {
        let body = b"wrong checksum".to_vec();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/package.exe", listener.local_addr().unwrap());
        let server = thread::spawn({
            let body = body.clone();
            move || serve_once(listener, body, "200 OK")
        });

        let directory = tempdir().unwrap();
        let destination = directory.path().join("package.exe");
        let release = release_for(&body, "0".repeat(64), url);
        let manager = DownloadManager::new(0).unwrap();
        let sink = RecordingSink(Arc::new(Mutex::new(Vec::new())));
        let error = manager
            .fetch(AppId::Codex, &release, &destination, &sink)
            .unwrap_err();

        server.join().unwrap();
        assert_eq!(error, InstallError::ChecksumMismatch);
        assert!(!destination.exists());
        assert!(!Path::new(&format!("{}.part", destination.display())).exists());
    }

    #[test]
    fn computes_sha256_for_file() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("data.bin");
        std::fs::write(&path, b"hello").unwrap();

        assert_eq!(sha256_file(&path).unwrap(), sha256_bytes(b"hello"));
    }
}
