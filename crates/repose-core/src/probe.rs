//! Repository URL liveness probe (Python `check_repo_url_async` baseline).

use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;

use crate::traits::Probe;

const SUFFIXES: &[&str] = &["repodata/repomd.xml", "suse/repodata/repomd.xml"];

/// Default async probe using rustls's platform verifier, so internal enterprise
/// mirrors signed by a private CA (e.g. the internal SUSE CA / "SUSE Trust
/// Root") validate just as they do under Python's system-CA probe.
#[derive(Debug, Clone)]
pub struct HttpProbe {
    /// `None` when client construction failed (e.g. the native root store
    /// could not be loaded): every probe then reports dead instead of the
    /// process aborting — see [`HttpProbe::default`].
    client: Option<Client>,
}

impl HttpProbe {
    fn new() -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()?;
        Ok(Self {
            client: Some(client),
        })
    }

    /// Probe that treats every URL as dead. Fallback when the HTTP client
    /// cannot be built; also usable in tests.
    #[must_use]
    const fn disabled() -> Self {
        Self { client: None }
    }

    /// Probe one base URL with HEAD→GET fallback per suffix.
    async fn check_url(&self, url: &str, timeout: Duration) -> bool {
        let Some(client) = &self.client else {
            return false;
        };
        for suffix in SUFFIXES {
            let target = format!("{url}{suffix}");
            match client.head(&target).timeout(timeout).send().await {
                Ok(resp) if resp.status().as_u16() < 400 => return true,
                Ok(_) => {
                    // Non-success HEAD *status* (>=400) retries with GET.
                    if let Ok(resp) = client.get(&target).timeout(timeout).send().await
                        && resp.status().as_u16() < 400
                    {
                        return true;
                    }
                }
                Err(_) => {
                    // HEAD transport error/timeout: Python probes HEAD and GET in
                    // one try/except, so the exception skips GET for this suffix.
                    // Match that — a mirror that fails HEAD at the transport layer
                    // will fail GET too, so the extra request is pure waste.
                }
            }
        }
        false
    }
}

impl Default for HttpProbe {
    /// Never panics: when the HTTP client cannot be built (e.g. unloadable
    /// native root store) the error is reported on stderr and a disabled
    /// probe is returned, so every URL probes dead instead of aborting the
    /// process mid-command.
    fn default() -> Self {
        match Self::new() {
            Ok(probe) => probe,
            Err(e) => {
                eprintln!("error: URL probe disabled, treating all repositories as dead: {e}");
                Self::disabled()
            }
        }
    }
}

#[async_trait]
impl Probe for HttpProbe {
    async fn is_live(&self, url: &str, timeout: Duration) -> bool {
        self.check_url(url, timeout).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn disabled_probe_reports_dead() {
        // Client-construction failure degrades to all-probes-dead, no panic.
        let probe = HttpProbe::disabled();
        assert!(
            !probe
                .check_url("http://example.invalid/", Duration::from_secs(1))
                .await
        );
    }

    #[tokio::test]
    async fn http_probe_head_ok() {
        let server = MockServer::start().await;
        Mock::given(method("HEAD"))
            .and(path("/repodata/repomd.xml"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let probe = HttpProbe::new().unwrap();
        let base = format!("{}/", server.uri());
        assert!(probe.check_url(&base, Duration::from_secs(2)).await);
    }

    #[tokio::test]
    async fn http_probe_head_fail_get_ok() {
        let server = MockServer::start().await;
        Mock::given(method("HEAD"))
            .and(path("/repodata/repomd.xml"))
            .respond_with(ResponseTemplate::new(405))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/repodata/repomd.xml"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let probe = HttpProbe::new().unwrap();
        let base = format!("{}/", server.uri());
        assert!(probe.check_url(&base, Duration::from_secs(2)).await);
    }

    #[tokio::test]
    async fn http_probe_head_status_matrix_falls_back_to_get() {
        for status in [400u16, 401, 403, 404, 500] {
            let server = MockServer::start().await;
            Mock::given(method("HEAD"))
                .and(path("/repodata/repomd.xml"))
                .respond_with(ResponseTemplate::new(status))
                .mount(&server)
                .await;
            Mock::given(method("GET"))
                .and(path("/repodata/repomd.xml"))
                .respond_with(ResponseTemplate::new(200))
                .mount(&server)
                .await;
            let probe = HttpProbe::new().unwrap();
            let base = format!("{}/", server.uri());
            assert!(
                probe.check_url(&base, Duration::from_secs(2)).await,
                "HEAD {status} should fall back to GET 200"
            );
        }
    }

    #[tokio::test]
    async fn http_probe_both_suffixes_fail_dead() {
        let server = MockServer::start().await;
        Mock::given(method("HEAD"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let probe = HttpProbe::new().unwrap();
        let base = format!("{}/", server.uri());
        assert!(!probe.check_url(&base, Duration::from_secs(2)).await);
    }

    #[tokio::test]
    async fn http_probe_second_suffix_fallback() {
        let server = MockServer::start().await;
        // Primary suffix dead (HEAD + GET 404); second suffix HEAD 200.
        Mock::given(method("HEAD"))
            .and(path("/repodata/repomd.xml"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/repodata/repomd.xml"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("HEAD"))
            .and(path("/suse/repodata/repomd.xml"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let probe = HttpProbe::new().unwrap();
        let base = format!("{}/", server.uri());
        assert!(probe.check_url(&base, Duration::from_secs(2)).await);
    }

    #[tokio::test]
    async fn http_probe_head_transport_error_skips_get() {
        // HEAD hangs far past the client timeout → transport error; GET would
        // succeed. Python (and the fixed Rust) must NOT fall back to GET after a
        // HEAD transport error, so the host stays dead.
        let server = MockServer::start().await;
        Mock::given(method("HEAD"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(30)))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let probe = HttpProbe::new().unwrap();
        let base = format!("{}/", server.uri());
        let live = probe.check_url(&base, Duration::from_millis(150)).await;
        assert!(!live, "HEAD transport error must not fall back to GET");
    }
}
