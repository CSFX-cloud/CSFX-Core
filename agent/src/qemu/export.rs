use anyhow::{Context, Result};
use axum::body::Bytes;
use futures_util::{stream, Stream, StreamExt};
use serde_json::json;
use std::{
    fmt::Display,
    io,
    path::{Path, PathBuf},
    pin::Pin,
    time::Duration,
};
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncWriteExt},
};
use tracing::warn;

use super::{qmp::QmpClient, runtime::boot_disk_path};

const BACKUP_JOB: &str = "csfx-export";
const BACKUP_NODE: &str = "csfx-export-target";
const DRIVE_DEVICE: &str = "virtio0";
const JOB_POLL: Duration = Duration::from_millis(500);
const READ_CHUNK: usize = 1 << 20;

pub type DiskStream = Pin<Box<dyn Stream<Item = io::Result<Bytes>> + Send>>;

struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_file(&self.0) {
            warn!(path = %self.0.display(), error = %error, "failed to remove export copy");
        }
    }
}

fn read_stream(file: fs::File, cleanup: Option<TempFile>) -> DiskStream {
    Box::pin(stream::try_unfold(
        (file, cleanup),
        |(mut file, cleanup)| async move {
            let mut buffer = Vec::with_capacity(READ_CHUNK);
            let read = (&mut file)
                .take(READ_CHUNK as u64)
                .read_to_end(&mut buffer)
                .await?;
            Ok((read > 0).then(|| (Bytes::from(buffer), (file, cleanup))))
        },
    ))
}

async fn run_backup(qmp: &mut QmpClient) -> Result<()> {
    let arguments = json!({
        "job-id": BACKUP_JOB,
        "device": DRIVE_DEVICE,
        "target": BACKUP_NODE,
        "sync": "full",
        "auto-dismiss": false,
    });
    qmp.send_command("blockdev-backup", Some(arguments)).await?;
    loop {
        let jobs = qmp.send_command("query-jobs", None).await?;
        let job = jobs
            .as_array()
            .and_then(|jobs| jobs.iter().find(|job| job["id"] == BACKUP_JOB))
            .cloned()
            .context("backup job disappeared")?;
        if job["status"] == "concluded" {
            qmp.send_command("job-dismiss", Some(json!({ "id": BACKUP_JOB })))
                .await?;
            return match job["error"].as_str() {
                Some(error) => anyhow::bail!("backup job failed: {error}"),
                None => Ok(()),
            };
        }
        tokio::time::sleep(JOB_POLL).await;
    }
}

async fn snapshot(qmp_socket: &Path, target: &Path) -> Result<()> {
    let mut qmp = QmpClient::connect(&qmp_socket.display().to_string()).await?;
    let node = json!({
        "driver": "raw",
        "node-name": BACKUP_NODE,
        "file": { "driver": "file", "filename": target },
    });
    qmp.send_command("blockdev-add", Some(node)).await?;
    let backup = run_backup(&mut qmp).await;
    let removed = qmp
        .send_command("blockdev-del", Some(json!({ "node-name": BACKUP_NODE })))
        .await;
    backup?;
    removed.map(drop)
}

pub async fn boot_disk_stream(
    workload_id: &str,
    qmp_socket: Option<PathBuf>,
) -> Result<Option<DiskStream>> {
    let disk = PathBuf::from(boot_disk_path(workload_id));
    let Some(qmp_socket) = qmp_socket else {
        return match fs::File::open(&disk).await {
            Ok(file) => Ok(Some(read_stream(file, None))),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        };
    };
    let size = fs::metadata(&disk).await?.len();
    let copy = disk.with_extension("export");
    let cleanup = TempFile(copy.clone());
    fs::File::create(&copy).await?.set_len(size).await?;
    snapshot(&qmp_socket, &copy).await?;
    let file = fs::File::open(&copy).await?;
    Ok(Some(read_stream(file, Some(cleanup))))
}

pub async fn import_boot_disk<S, E>(workload_id: &str, mut source: S) -> Result<bool>
where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
    E: Display,
{
    let disk = PathBuf::from(boot_disk_path(workload_id));
    if fs::metadata(&disk).await.is_ok() {
        return Ok(false);
    }
    if let Some(parent) = disk.parent() {
        fs::create_dir_all(parent).await?;
    }
    let partial = disk.with_extension("partial");
    let result = write_partial(&partial, &mut source).await;
    let outcome = match result {
        Ok(()) => publish(&partial, &disk).await,
        Err(error) => Err(error),
    };
    if let Err(error) = fs::remove_file(&partial).await {
        if error.kind() != io::ErrorKind::NotFound {
            warn!(path = %partial.display(), error = %error, "failed to remove partial import");
        }
    }
    outcome
}

async fn write_partial<S, E>(partial: &Path, source: &mut S) -> Result<()>
where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
    E: Display,
{
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(partial)
        .await?;
    while let Some(chunk) = source.next().await {
        let chunk = chunk.map_err(|error| anyhow::anyhow!("{error}"))?;
        file.write_all(&chunk).await?;
    }
    file.sync_all().await?;
    Ok(())
}

async fn publish(partial: &Path, disk: &Path) -> Result<bool> {
    match fs::hard_link(partial, disk).await {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncBufReadExt, BufReader},
        net::UnixListener,
    };

    async fn serve_qmp(path: &Path, replies: &'static [&'static str]) {
        let listener = UnixListener::bind(path).unwrap();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (read_half, mut write_half) = stream.into_split();
            let mut reader = BufReader::new(read_half);
            write_half.write_all(b"{\"QMP\":{}}\n").await.unwrap();
            for reply in replies {
                let mut request = String::new();
                reader.read_line(&mut request).await.unwrap();
                write_half.write_all(reply.as_bytes()).await.unwrap();
            }
        });
    }

    fn scratch(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("csfx-export-{name}-{}", uuid::Uuid::new_v4()))
    }

    const SUCCESS: &[&str] = &[
        "{\"return\":{}}\n",
        "{\"return\":{}}\n",
        "{\"return\":{}}\n",
        "{\"return\":[{\"id\":\"csfx-export\",\"status\":\"running\"}]}\n",
        "{\"return\":[{\"id\":\"csfx-export\",\"status\":\"concluded\"}]}\n",
        "{\"return\":{}}\n",
        "{\"return\":{}}\n",
    ];

    #[tokio::test]
    async fn snapshot_runs_the_backup_job_to_completion() {
        let socket = scratch("ok.sock");
        serve_qmp(&socket, SUCCESS).await;
        snapshot(&socket, Path::new("/tmp/target.img"))
            .await
            .unwrap();
        std::fs::remove_file(&socket).unwrap();
    }

    #[tokio::test]
    async fn snapshot_reports_a_failed_job_and_still_removes_the_node() {
        let socket = scratch("fail.sock");
        serve_qmp(
            &socket,
            &[
                "{\"return\":{}}\n",
                "{\"return\":{}}\n",
                "{\"return\":{}}\n",
                "{\"return\":[{\"id\":\"csfx-export\",\"status\":\"concluded\",\"error\":\"No space left on device\"}]}\n",
                "{\"return\":{}}\n",
                "{\"return\":{}}\n",
            ],
        )
        .await;
        let error = snapshot(&socket, Path::new("/tmp/target.img"))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("No space left"));
        std::fs::remove_file(&socket).unwrap();
    }

    #[tokio::test]
    async fn read_stream_returns_the_whole_file_and_removes_the_copy() {
        let path = scratch("copy.img");
        let content: Vec<u8> = (0..(READ_CHUNK * 2 + 7)).map(|n| (n % 251) as u8).collect();
        std::fs::write(&path, &content).unwrap();
        let file = fs::File::open(&path).await.unwrap();
        let mut stream = read_stream(file, Some(TempFile(path.clone())));
        let mut read = Vec::new();
        while let Some(chunk) = stream.next().await {
            read.extend_from_slice(&chunk.unwrap());
        }
        assert_eq!(read, content);
        drop(stream);
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn partial_import_is_published_without_overwriting() {
        let partial = scratch("partial.img");
        let disk = scratch("final.img");
        let chunks: Vec<Result<Bytes, String>> = vec![
            Ok(Bytes::from_static(b"abc")),
            Ok(Bytes::from_static(b"def")),
        ];
        write_partial(&partial, &mut stream::iter(chunks))
            .await
            .unwrap();
        assert!(publish(&partial, &disk).await.unwrap());
        assert_eq!(std::fs::read(&disk).unwrap(), b"abcdef");
        assert!(!publish(&partial, &disk).await.unwrap());
        std::fs::remove_file(&partial).unwrap();
        std::fs::remove_file(&disk).unwrap();
    }

    #[tokio::test]
    async fn broken_source_aborts_the_import() {
        let partial = scratch("broken.img");
        let chunks: Vec<Result<Bytes, String>> = vec![
            Ok(Bytes::from_static(b"abc")),
            Err("upstream closed".to_string()),
        ];
        let error = write_partial(&partial, &mut stream::iter(chunks))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("upstream closed"));
        std::fs::remove_file(&partial).unwrap();
    }
}
