//! Refuse to run the server-driving integration tests against a live sccache server.
//!
//! The integration tests start and stop real sccache servers. `tests/harness`
//! strips every `SCCACHE_*` variable from the commands it runs, including
//! `SCCACHE_SERVER_PORT`, so those tests always use the default port and cannot
//! be isolated from a developer's own server by environment variables. Run them
//! on a machine that has one and they stop it, serve its clients from a test
//! daemon with a temporary cache, and leave it for whoever compiles next to
//! restart.
//!
//! Before the first server command of each test binary, the harnesses call
//! [`refuse_live_server`]. If something already listens on the port the tests
//! are about to use, it panics with instructions instead. Set
//! `SCCACHE_TEST_ALLOW_LIVE=1` to override (for example in a CI container where
//! a leftover server from an earlier step is expected).

use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::sync::Mutex;
use std::time::Duration;

/// The port an sccache client uses when `SCCACHE_SERVER_PORT` is unset. Mirrors
/// `DEFAULT_PORT` in src/commands.rs (private to the crate);
/// tests/live_server_guard.rs fails if the two drift apart.
#[allow(dead_code)]
pub const DEFAULT_PORT: u16 = 4226;

/// Set to `1` to run the tests even though a server already listens.
pub const ALLOW_VAR: &str = "SCCACHE_TEST_ALLOW_LIVE";

/// The port a command that inherits this process's environment will use.
#[allow(dead_code)]
pub fn inherited_port() -> u16 {
    std::env::var("SCCACHE_SERVER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

/// `Err` with an explanation when something listens on `port` and `allow` is false.
pub fn check(port: u16, allow: bool) -> Result<(), String> {
    if allow {
        return Ok(());
    }
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    match TcpStream::connect_timeout(&addr, Duration::from_millis(250)) {
        Ok(_) => Err(format!(
            "a server is already listening on 127.0.0.1:{port}. These tests stop and start \
             sccache servers on that port, so running them would stop it (tests/harness also \
             strips SCCACHE_SERVER_PORT, so an environment variable cannot move them elsewhere). \
             Run them in a container, a VM or a separate user account, or set {ALLOW_VAR}=1 if \
             stopping that server is fine."
        )),
        Err(_) => Ok(()),
    }
}

/// Panic unless nothing listens on `port` or `SCCACHE_TEST_ALLOW_LIVE=1` is set.
///
/// Checked once per port per test binary: after the first check passes, the
/// tests' own servers are expected to come and go on that port, so a later
/// listener cannot be told apart from them. A server that some other program
/// starts on the port while the tests run is therefore not detected; that is
/// why the README says to run these tests in a sandbox rather than rely on this.
#[allow(dead_code)]
pub fn refuse_live_server(port: u16) {
    static CHECKED: Mutex<Vec<u16>> = Mutex::new(Vec::new());
    let mut checked = CHECKED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if checked.contains(&port) {
        return;
    }
    let allow = std::env::var(ALLOW_VAR).is_ok_and(|v| v == "1");
    if let Err(message) = check(port, allow) {
        panic!("{message}");
    }
    checked.push(port);
}
