use axum::body::Bytes;
use futures_util::stream;
use tokio::{
    io::AsyncReadExt,
    process::{Child, ChildStdout, Command},
    task::JoinHandle,
};

use crate::export::{ByteStream, ExportError};

const READ_CHUNK: usize = 1 << 20;
const STDERR_LIMIT: u64 = 8 << 10;

struct Running {
    stdout: ChildStdout,
    child: Child,
    stderr: JoinHandle<String>,
    label: &'static str,
}

async fn collect_stderr(mut pipe: tokio::process::ChildStderr) -> String {
    let mut text = String::new();
    let mut limited = (&mut pipe).take(STDERR_LIMIT);
    if limited.read_to_string(&mut text).await.is_err() {
        text.push_str("stderr was not valid text");
    }
    let mut sink = Vec::new();
    let _ = pipe.read_to_end(&mut sink).await;
    text
}

async fn next_chunk(mut state: Running) -> Result<Option<(Bytes, Running)>, ExportError> {
    let mut buffer = Vec::with_capacity(READ_CHUNK);
    let read = (&mut state.stdout)
        .take(READ_CHUNK as u64)
        .read_to_end(&mut buffer)
        .await
        .map_err(|error| ExportError::Upstream(format!("{} output: {error}", state.label)))?;
    if read > 0 {
        return Ok(Some((Bytes::from(buffer), state)));
    }
    let status = state
        .child
        .wait()
        .await
        .map_err(|error| ExportError::Upstream(format!("{}: {error}", state.label)))?;
    let stderr = state.stderr.await.unwrap_or_default();
    if status.success() {
        return Ok(None);
    }
    Err(ExportError::Upstream(format!(
        "{} exited with {status}: {}",
        state.label,
        stderr.trim()
    )))
}

pub fn command_stream(
    mut command: Command,
    label: &'static str,
) -> Result<ByteStream, ExportError> {
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|error| ExportError::Upstream(format!("{label} could not start: {error}")))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ExportError::Upstream(format!("{label} has no stdout")))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| ExportError::Upstream(format!("{label} has no stderr")))?;
    let state = Running {
        stdout,
        child,
        stderr: tokio::spawn(collect_stderr(stderr)),
        label,
    };
    Ok(Box::pin(stream::try_unfold(state, next_chunk)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::pipeline::collect;

    #[tokio::test]
    async fn streams_stdout_of_a_successful_command() {
        let mut command = Command::new("sh");
        command.args(["-c", "printf abc; printf def"]);
        let output = collect(command_stream(command, "test").unwrap())
            .await
            .unwrap();
        assert_eq!(output, b"abcdef");
    }

    #[tokio::test]
    async fn failing_commands_surface_stderr() {
        let mut command = Command::new("sh");
        command.args(["-c", "echo partial; echo broken >&2; exit 3"]);
        let error = collect(command_stream(command, "dump").unwrap())
            .await
            .unwrap_err();
        let message = error.to_string();
        assert!(message.contains("dump exited"), "{message}");
        assert!(message.contains("broken"), "{message}");
    }

    #[tokio::test]
    async fn missing_binaries_are_reported() {
        let command = Command::new("/nonexistent/csfx-binary");
        assert!(command_stream(command, "dump").is_err());
    }
}
