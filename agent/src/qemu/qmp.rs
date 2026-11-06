use anyhow::{Context, Result};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

pub struct QmpClient {
    reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    writer: tokio::net::unix::OwnedWriteHalf,
}

impl QmpClient {
    pub async fn connect(socket_path: &str) -> Result<Self> {
        let stream = connect_with_retry(socket_path).await?;
        let (read_half, write_half) = stream.into_split();

        let mut client = Self {
            reader: BufReader::new(read_half),
            writer: write_half,
        };

        client
            .read_line_json()
            .await
            .context("Failed to read QMP greeting")?;

        client
            .send_command("qmp_capabilities", None)
            .await
            .context("Failed to negotiate QMP capabilities")?;

        Ok(client)
    }

    pub async fn send_command(&mut self, execute: &str, arguments: Option<Value>) -> Result<Value> {
        let mut command = json!({ "execute": execute });
        if let Some(arguments) = arguments {
            command["arguments"] = arguments;
        }

        let mut payload =
            serde_json::to_vec(&command).context("Failed to serialize QMP command")?;
        payload.push(b'\n');

        self.writer
            .write_all(&payload)
            .await
            .context("Failed to write QMP command")?;

        loop {
            let response = self.read_line_json().await?;
            if let Some(error) = response.get("error") {
                anyhow::bail!("QMP command {} failed: {}", execute, error);
            }
            if let Some(return_value) = response.get("return") {
                return Ok(return_value.clone());
            }
        }
    }

    async fn read_line_json(&mut self) -> Result<Value> {
        let mut line = String::new();
        loop {
            line.clear();
            let bytes_read = self
                .reader
                .read_line(&mut line)
                .await
                .context("Failed to read QMP response line")?;
            if bytes_read == 0 {
                anyhow::bail!("QMP socket closed before response");
            }

            let parsed: Value =
                serde_json::from_str(line.trim()).context("Failed to parse QMP response")?;

            if parsed.get("event").is_some() {
                continue;
            }

            return Ok(parsed);
        }
    }
}

async fn connect_with_retry(socket_path: &str) -> Result<UnixStream> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);

    loop {
        match UnixStream::connect(socket_path).await {
            Ok(stream) => return Ok(stream),
            Err(e) if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                let _ = e;
            }
            Err(e) => {
                return Err(e)
                    .with_context(|| format!("Failed to connect to QMP socket {}", socket_path))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::UnixListener;

    fn socket_path() -> String {
        std::env::temp_dir()
            .join(format!("csfx-qmp-{}.sock", uuid::Uuid::new_v4()))
            .display()
            .to_string()
    }

    async fn serve(path: &str, script: &'static [&'static str]) {
        let listener = UnixListener::bind(path).unwrap();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (read_half, mut write_half) = stream.into_split();
            let mut reader = BufReader::new(read_half);
            write_half
                .write_all(b"{\"QMP\":{\"version\":{}}}\n")
                .await
                .unwrap();
            for reply in script {
                let mut request = String::new();
                reader.read_line(&mut request).await.unwrap();
                write_half.write_all(reply.as_bytes()).await.unwrap();
            }
        });
    }

    #[tokio::test]
    async fn commands_skip_events_and_return_values() {
        let path = socket_path();
        serve(
            &path,
            &[
                "{\"return\":{}}\n",
                "{\"event\":\"STOP\"}\n{\"return\":{\"status\":\"running\"}}\n",
                "{\"error\":{\"desc\":\"bad\"}}\n",
            ],
        )
        .await;
        let mut client = QmpClient::connect(&path).await.unwrap();
        let status = client.send_command("query-status", None).await.unwrap();
        assert_eq!(status["status"], "running");
        let failure = client
            .send_command("balloon", Some(json!({"value": 1})))
            .await;
        assert!(failure.unwrap_err().to_string().contains("balloon"));
        std::fs::remove_file(&path).unwrap();
    }

    #[tokio::test]
    async fn closed_sockets_are_reported() {
        let path = socket_path();
        serve(&path, &["{\"return\":{}}\n"]).await;
        let mut client = QmpClient::connect(&path).await.unwrap();
        assert!(client.send_command("quit", None).await.is_err());
        std::fs::remove_file(&path).unwrap();
    }

    #[tokio::test]
    async fn malformed_responses_are_errors() {
        let path = socket_path();
        serve(&path, &["garbage\n"]).await;
        assert!(QmpClient::connect(&path).await.is_err());
        std::fs::remove_file(&path).unwrap();
    }
}
