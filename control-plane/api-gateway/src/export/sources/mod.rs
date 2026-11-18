pub mod bucket;
pub mod config;
pub mod volume;

use futures_util::StreamExt;
use std::fmt::Display;

use super::{ByteStream, ExportError};

pub fn upstream(error: impl Display) -> ExportError {
    ExportError::Upstream(error.to_string())
}

pub fn response_stream(response: reqwest::Response) -> ByteStream {
    Box::pin(response.bytes_stream().map(|chunk| chunk.map_err(upstream)))
}

pub fn request_body(stream: ByteStream) -> reqwest::Body {
    reqwest::Body::wrap_stream(stream)
}
