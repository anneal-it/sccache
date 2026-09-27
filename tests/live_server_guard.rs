//! Tests for the live-server guard the integration harnesses call before
//! touching a server (tests/guard). These only ever probe ephemeral ports.

mod guard;

use std::net::TcpListener;

fn free_port() -> u16 {
    // Bind and drop: nothing listens on the port afterwards.
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[test]
fn refuses_when_a_server_listens() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let err = guard::check(port, false).unwrap_err();
    assert!(err.contains(&format!("127.0.0.1:{port}")), "{err}");
    assert!(err.contains(guard::ALLOW_VAR), "{err}");
}

#[test]
fn allows_an_idle_port() {
    assert_eq!(guard::check(free_port(), false), Ok(()));
}

#[test]
fn allows_a_listening_port_when_overridden() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    assert_eq!(guard::check(port, true), Ok(()));
}

#[test]
fn default_port_matches_the_client() {
    // src/commands.rs keeps DEFAULT_PORT private; pin the copy the guard uses.
    let commands = include_str!("../src/commands.rs");
    let expected = format!("pub const DEFAULT_PORT: u16 = {};", guard::DEFAULT_PORT);
    assert!(
        commands.contains(&expected),
        "tests/guard DEFAULT_PORT drifted from src/commands.rs"
    );
}

#[cfg(unix)]
#[test]
fn refuses_a_listening_unix_socket() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sccache.sock");
    let _listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
    let err = guard::check_endpoint(&guard::Endpoint::Unix(path.clone()), false).unwrap_err();
    assert!(err.contains(&path.display().to_string()), "{err}");
}

#[cfg(unix)]
#[test]
fn allows_a_unix_socket_nobody_listens_on() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("absent.sock");
    assert_eq!(
        guard::check_endpoint(&guard::Endpoint::Unix(path), false),
        Ok(())
    );
}

#[test]
fn refuses_an_abstract_socket_it_cannot_probe_unless_overridden() {
    let abstract_socket = guard::Endpoint::UnixAbstract("sccache".to_owned());
    assert!(guard::check_endpoint(&abstract_socket, false).is_err());
    assert_eq!(guard::check_endpoint(&abstract_socket, true), Ok(()));
}
