//! Repository URL liveness probe (Python `check_repo_url_async` baseline).

use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;

use crate::traits::Probe;

const SUFFIXES: &[&str] = &["repodata/repomd.xml", "suse/repodata/repomd.xml"];

/// Default async probe using system-ish TLS (rustls webpki roots for now;
/// native system CA wiring can tighten later for enterprise mirrors).
#[derive(Debug, Clone)]
pub struct HttpProbe {
    client: Client,
}

impl HttpProbe {
    pub fn new() -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()?;
        Ok(Self { client })
    }

    /// Probe one base URL with HEAD→GET fallback per suffix.
    pub async fn check_url(&self, url: &str, timeout: Duration) -> bool {
        for suffix in SUFFIXES {
            let target = format!("{url}{suffix}");
            match self.client.head(&target).timeout(timeout).send().await {
                Ok(resp) if resp.status().as_u16() < 400 => return true,
                Ok(_) => {
                    // Non-success HEAD *status* (>=400) retries with GET.
                    if let Ok(resp) = self.client.get(&target).timeout(timeout).send().await {
                        if resp.status().as_u16() < 400 {
                            return true;
                        }
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
    fn default() -> Self {
        Self::new().expect("reqwest client")
    }
}

#[async_trait]
impl Probe for HttpProbe {
    async fn is_live(&self, url: &str, timeout: Duration) -> bool {
        self.check_url(url, timeout).await
    }
}

/// Filter URLs preserving order; drop dead ones.
pub async fn filter_live_urls(
    probe: &dyn Probe,
    urls: &[String],
    timeout: Duration,
    no_probe: bool,
) -> Vec<String> {
    if no_probe {
        return urls.to_vec();
    }
    let mut out = Vec::new();
    for u in urls {
        if probe.is_live(u, timeout).await {
            out.push(u.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::ConstProbe;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn const_probe_order_preserved() {
        let p = ConstProbe { live: true };
        let urls = vec!["http://a/".into(), "http://b/".into()];
        let live = filter_live_urls(&p, &urls, Duration::from_secs(1), false).await;
        assert_eq!(live, urls);
    }

    #[tokio::test]
    async fn no_probe_short_circuit() {
        let p = ConstProbe { live: false };
        let urls = vec!["http://a/".into()];
        let live = filter_live_urls(&p, &urls, Duration::from_secs(1), true).await;
        assert_eq!(live, urls);
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
