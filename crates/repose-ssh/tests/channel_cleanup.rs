//! Regression guard for the timed-out-channel cleanup: a `run()` that hits
//! its command deadline must send `SSH_MSG_CHANNEL_CLOSE` for the channel it
//! abandons, and must leave the transport usable.
//!
//! This asserts the wire behaviour directly, against an in-process
//! `russh::server`, because that is the only place it is observable. A real
//! OpenSSH peer cannot witness it: `session_close_by_channel()` returns early
//! while the exec child is still alive (`s->pid != 0` with `force == 0`), so
//! the `MaxSessions` slot is released on child reap, not on channel close.
//! The fake server never reaps anything, so a recorded `channel_close`
//! callback is exactly the client-side send under test.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use repose_core::error::{SshError, TimeoutPhase};
use repose_core::{ConnectionConfig, HostKeyPolicy};
use repose_ssh::{RusshSession, SshSession};
use russh::keys::PrivateKey;
use russh::keys::ssh_key::LineEnding;
use russh::keys::ssh_key::private::Ed25519Keypair;
use russh::server::{Auth, Config, Handler, Msg, Session};
use russh::{Channel, ChannelId};
use tokio::net::TcpListener;

/// The command the fake server deliberately never completes, so the
/// client's command deadline is the only way out of `run()`.
const HANGING_COMMAND: &str = "hang";

/// Channel bookkeeping shared between the fake server and the assertions.
#[derive(Clone, Default)]
struct ChannelLog {
    opened: Arc<Mutex<Vec<ChannelId>>>,
    closed: Arc<Mutex<Vec<ChannelId>>>,
}

impl ChannelLog {
    fn record_open(&self, id: ChannelId) {
        self.opened
            .lock()
            .expect("channel-open log should not be poisoned")
            .push(id);
    }

    fn record_close(&self, id: ChannelId) {
        self.closed
            .lock()
            .expect("channel-close log should not be poisoned")
            .push(id);
    }

    fn first_opened(&self) -> Option<ChannelId> {
        self.opened
            .lock()
            .expect("channel-open log should not be poisoned")
            .first()
            .copied()
    }

    fn was_closed(&self, id: ChannelId) -> bool {
        self.closed
            .lock()
            .expect("channel-close log should not be poisoned")
            .contains(&id)
    }
}

/// Accepts any public key and any session channel; answers every `exec`
/// except [`HANGING_COMMAND`], which is acknowledged and then left pending
/// forever (no data, no `exit-status`, no close).
struct FakeServer {
    log: ChannelLog,
}

impl Handler for FakeServer {
    type Error = russh::Error;

    async fn auth_publickey(
        &mut self,
        _user: &str,
        _public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<Auth, Self::Error> {
        Ok(Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: russh::server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.log.record_open(channel.id());
        reply.accept().await;
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        if data == HANGING_COMMAND.as_bytes() {
            return Ok(());
        }
        session.data(channel, &b"ok"[..])?;
        session.exit_status_request(channel, 0)?;
        session.eof(channel)?;
        session.close(channel)?;
        Ok(())
    }

    async fn channel_close(
        &mut self,
        channel: ChannelId,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.log.record_close(channel);
        Ok(())
    }
}

/// Deterministic throwaway keys: `PrivateKey::random` would pull `rand` in as
/// a dev-dependency, and a fixed seed is equally valid for a key that never
/// leaves this process.
fn test_key(seed: u8) -> PrivateKey {
    Ed25519Keypair::from_seed(&[seed; 32]).into()
}

/// Bind an ephemeral port, serve exactly one connection with [`FakeServer`],
/// and return the port plus the shared channel log.
async fn start_fake_server() -> (u16, ChannelLog) {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("ephemeral loopback port should bind");
    let port = listener
        .local_addr()
        .expect("bound listener should report its address")
        .port();
    let log = ChannelLog::default();
    let handler_log = log.clone();

    tokio::spawn(async move {
        let config = Arc::new(Config {
            keys: vec![test_key(1)],
            inactivity_timeout: None,
            ..Config::default()
        });
        let Ok((stream, _)) = listener.accept().await else {
            return;
        };
        let Ok(session) =
            russh::server::run_stream(config, stream, FakeServer { log: handler_log }).await
        else {
            return;
        };
        let _ = session.await;
    });

    (port, log)
}

/// Poll `condition` until it holds or `budget` elapses; returns whether it
/// held. The close travels over a real socket, so the assertion cannot be
/// synchronous with `run()` returning.
async fn wait_for(budget: Duration, condition: impl Fn() -> bool) -> bool {
    let deadline = tokio::time::Instant::now() + budget;
    while tokio::time::Instant::now() < deadline {
        if condition() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    condition()
}

#[tokio::test]
async fn a_timed_out_command_closes_its_channel_and_keeps_the_session_usable() {
    let (port, log) = start_fake_server().await;
    let temp = tempfile::tempdir().expect("temporary key directory should be created");
    let identity = temp.path().join("id_ed25519");
    std::fs::write(
        &identity,
        test_key(2)
            .to_openssh(LineEnding::LF)
            .expect("throwaway client key should encode")
            .as_str(),
    )
    .expect("throwaway client key should be written");

    let config = ConnectionConfig {
        host_key_policy: HostKeyPolicy::Off,
        known_hosts: None,
        // One budget serves both phases: long enough that the *completing*
        // command below cannot lose a race with a loaded CI runner, short
        // enough that deliberately burning it once keeps the test ~1s.
        timeout: 1.0,
        ..ConnectionConfig::default()
    };
    let mut session =
        RusshSession::new("127.0.0.1", port, "tester", config).with_identity(identity);
    session
        .connect()
        .await
        .expect("the in-process server should accept the throwaway key");

    let error = session
        .run(HANGING_COMMAND)
        .await
        .expect_err("a command the server never completes must hit the command deadline");
    assert!(
        matches!(
            error,
            SshError::Timeout {
                phase: TimeoutPhase::Command,
                ..
            }
        ),
        "unexpected error: {error:?}"
    );

    let hung = log
        .first_opened()
        .expect("the server should have seen the channel open");
    // Asserted before the session is dropped: a close observed only after
    // teardown would prove nothing about the timeout path.
    assert!(
        wait_for(Duration::from_secs(5), || log.was_closed(hung)).await,
        "the timed-out channel was abandoned instead of closed"
    );
    assert!(session.is_active());

    let (stdout, _, status) = session
        .run("finish")
        .await
        .expect("closing the timed-out channel must not poison the transport");
    assert_eq!((stdout.as_str(), status), ("ok", 0));

    session.close().await.expect("session should close");
}
