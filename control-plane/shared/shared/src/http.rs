use reqwest::{Client, ClientBuilder};
use std::time::Duration;

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

pub fn builder(timeout: Duration) -> ClientBuilder {
    Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(timeout)
}

pub fn client(timeout: Duration) -> Client {
    builder(timeout)
        .build()
        .expect("failed to build http client")
}
