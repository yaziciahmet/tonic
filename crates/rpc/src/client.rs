use std::time::Duration;

use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};

pub fn create_http_client(server_url: &str, config: HttpClientConfig) -> HttpClient {
    HttpClientBuilder::new()
        .request_timeout(config.timeout)
        .build(server_url)
        .expect("Failed to create http client")
}

pub struct HttpClientConfig {
    timeout: Duration,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(60),
        }
    }
}

impl HttpClientConfig {
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}
