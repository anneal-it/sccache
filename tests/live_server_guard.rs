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
fn the_override_allows_a_listening_port() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    assert_eq!(guard::check(port, true), Ok(()));
}
