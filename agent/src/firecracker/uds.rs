use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use tokio::net::UnixStream;

const MAX_DIRECT_SOCKET_PATH_BYTES: usize = 100;

pub fn via_directory_fd(directory: &std::fs::File, name: &std::ffi::OsStr) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd())).join(name)
}

pub async fn connect(path: &Path) -> std::io::Result<UnixStream> {
    let direct = path.as_os_str().len() <= MAX_DIRECT_SOCKET_PATH_BYTES;
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return UnixStream::connect(path).await;
    };
    if direct {
        return UnixStream::connect(path).await;
    }
    let directory = std::fs::File::open(parent)?;
    let reachable = via_directory_fd(&directory, name);
    let stream = UnixStream::connect(&reachable).await;
    drop(directory);
    stream
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixListener;

    fn long_directory() -> PathBuf {
        let mut directory = std::env::temp_dir().join(format!("csfx-uds-{}", uuid::Uuid::new_v4()));
        while directory.as_os_str().len() <= MAX_DIRECT_SOCKET_PATH_BYTES + 20 {
            directory.push("a-long-directory-name");
        }
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[tokio::test]
    async fn sockets_beyond_the_unix_path_limit_stay_reachable() {
        let directory = long_directory();
        let path = directory.join("long.socket");
        assert!(path.as_os_str().len() > MAX_DIRECT_SOCKET_PATH_BYTES);
        let handle = std::fs::File::open(&directory).unwrap();
        let listener =
            UnixListener::bind(via_directory_fd(&handle, "long.socket".as_ref())).unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            socket.write_all(b"pong").await.unwrap();
        });

        let mut stream = connect(&path).await.unwrap();
        let mut reply = Vec::new();
        stream.read_to_end(&mut reply).await.unwrap();

        assert_eq!(reply, b"pong");
        let root = directory
            .ancestors()
            .find(|ancestor| {
                ancestor
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("csfx-uds-"))
            })
            .unwrap()
            .to_path_buf();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn short_paths_connect_directly() {
        let path = std::env::temp_dir().join(format!(
            "u{}.sock",
            &uuid::Uuid::new_v4().simple().to_string()[..8]
        ));
        let listener = UnixListener::bind(&path).unwrap();
        tokio::spawn(async move {
            listener.accept().await.unwrap();
        });
        assert!(connect(&path).await.is_ok());
        std::fs::remove_file(path).unwrap();
    }
}
