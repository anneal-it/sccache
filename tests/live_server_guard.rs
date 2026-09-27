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

#[cfg(unix)]
#[test]
fn refuses_a_socket_it_cannot_probe() {
    // A socket path under a directory the test cannot search: connecting fails
    // with PermissionDenied, which does not prove the socket is idle.
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let locked = dir.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    let path = locked.join("sccache.sock");
    let _listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    let result = guard::check_endpoint(&guard::Endpoint::Unix(path), false);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).unwrap();
    // Root can search any directory; then the probe connects and refuses anyway.
    assert!(result.is_err(), "{result:?}");
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[test]
fn probes_abstract_sockets() {
    #[cfg(target_os = "android")]
    use std::os::android::net::SocketAddrExt;
    #[cfg(target_os = "linux")]
    use std::os::linux::net::SocketAddrExt;
    let name = format!("sccache-guard-test-{}", std::process::id()).into_bytes();
    let addr = std::os::unix::net::SocketAddr::from_abstract_name(&name).unwrap();
    let listener = std::os::unix::net::UnixListener::bind_addr(&addr).unwrap();
    assert!(guard::check_endpoint(&guard::Endpoint::UnixAbstract(name.clone()), false).is_err());
    drop(listener);
    assert_eq!(
        guard::check_endpoint(&guard::Endpoint::UnixAbstract(name), false),
        Ok(())
    );
}
