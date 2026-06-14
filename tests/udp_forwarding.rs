use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TestEnv {
    root: PathBuf,
    config_dir: PathBuf,
    data_dir: PathBuf,
    udp_idle_timeout_ms: Option<u64>,
}

impl TestEnv {
    fn new(name: &str) -> Self {
        let unique = format!(
            "portcli-{}-{}-{}-{}",
            name,
            std::process::id(),
            TEST_COUNTER.fetch_add(1, Ordering::SeqCst),
            unique_suffix()
        );
        let root = std::env::temp_dir().join(unique);
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        fs::create_dir_all(&config_dir).unwrap();
        fs::create_dir_all(&data_dir).unwrap();

        Self {
            root,
            config_dir,
            data_dir,
            udp_idle_timeout_ms: None,
        }
    }

    fn with_udp_idle_timeout_ms(mut self, millis: u64) -> Self {
        self.udp_idle_timeout_ms = Some(millis);
        self
    }

    fn run(&self, args: &[&str]) -> Output {
        let output = self.command(args).output().unwrap();
        assert!(
            output.status.success(),
            "command failed: {:?}\nstdout:\n{}\nstderr:\n{}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    fn try_run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }

    fn run_daemon(&self) {
        let status = self
            .command(&["run"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "failed to start daemon");
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_portcli"));
        cmd.args(args)
            .env("PORTCLI_CONFIG_DIR", &self.config_dir)
            .env("PORTCLI_DATA_DIR", &self.data_dir);
        if let Some(millis) = self.udp_idle_timeout_ms {
            cmd.env("PORTCLI_UDP_SESSION_IDLE_TIMEOUT_MS", millis.to_string());
        }
        cmd
    }

    fn stdout(&self, args: &[&str]) -> String {
        String::from_utf8_lossy(&self.run(args).stdout).to_string()
    }

    fn config_path(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }

    fn rule_log_path(&self, name: &str) -> PathBuf {
        self.data_dir
            .join("logs")
            .join("rules")
            .join(format!("{}.log", name))
    }

    fn stop(&self) {
        let _ = self
            .command(&["stop"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        self.stop();
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn cli_config_protocols_and_legacy_config_work() {
    let env = TestEnv::new("cli-config");

    env.run(&[
        "add",
        "web",
        "--source",
        "127.0.0.1:10000",
        "--target",
        "127.0.0.1:10001",
    ]);
    env.run(&[
        "add",
        "dns",
        "--source",
        "127.0.0.1:1053",
        "--target",
        "127.0.0.1:53",
        "--protocol",
        "udp",
    ]);

    let config = fs::read_to_string(env.config_path()).unwrap();
    assert!(config.contains("protocol = \"tcp\""));
    assert!(config.contains("protocol = \"udp\""));

    let list = env.stdout(&["list"]);
    assert!(list.contains("web"));
    assert!(list.contains("tcp"));
    assert!(list.contains("dns"));
    assert!(list.contains("udp"));

    let status = env.stdout(&["status"]);
    assert!(status.contains("daemon: not running"));
    assert!(status.contains("tcp"));
    assert!(status.contains("udp"));

    env.run(&["modify", "web", "--protocol", "udp"]);
    let modified = fs::read_to_string(env.config_path()).unwrap();
    assert!(modified.contains("name = \"web\"\nprotocol = \"udp\""));

    let no_changes = env.try_run(&["modify", "web"]);
    assert!(!no_changes.status.success());
    assert!(String::from_utf8_lossy(&no_changes.stderr).contains("no changes specified"));

    let legacy = TestEnv::new("legacy-config");
    fs::write(
        legacy.config_path(),
        r#"
[[rules]]
name = "legacy"
source = "127.0.0.1:20000"
target = "127.0.0.1:20001"
enabled = true
"#,
    )
    .unwrap();

    let legacy_status = legacy.stdout(&["status"]);
    assert!(legacy_status.contains("legacy"));
    assert!(legacy_status.contains("tcp"));
}

#[test]
fn tcp_round_trip_still_works() {
    let env = TestEnv::new("tcp-round-trip");
    let source_port = free_tcp_port();
    let backend = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    backend.set_nonblocking(true).unwrap();
    let target_port = backend.local_addr().unwrap().port();
    let (received_tx, received_rx) = mpsc::channel();

    env.run(&[
        "add",
        "tcp",
        "--source",
        &format!("127.0.0.1:{source_port}"),
        "--target",
        &format!("127.0.0.1:{target_port}"),
    ]);
    env.run(&["enable", "tcp"]);
    env.run_daemon();
    wait_for_status(&env, &["tcp", "protocol: tcp", "status: running"]);

    thread::spawn(move || {
        let (mut stream, _) = accept_with_timeout(&backend);
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut buf = [0_u8; 1024];
        let n = stream.read(&mut buf).unwrap();
        received_tx.send(buf[..n].to_vec()).unwrap();
        stream.write_all(b"PONG_FROM_TCP").unwrap();
    });

    let source_addr: SocketAddr = format!("127.0.0.1:{source_port}").parse().unwrap();
    let mut client = TcpStream::connect_timeout(&source_addr, Duration::from_secs(3)).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    client
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    client.write_all(b"PING_FROM_TCP").unwrap();

    let mut reply = [0_u8; 13];
    client.read_exact(&mut reply).unwrap();
    assert_eq!(&reply, b"PONG_FROM_TCP");
    assert_eq!(
        received_rx.recv_timeout(Duration::from_secs(3)).unwrap(),
        b"PING_FROM_TCP"
    );

    env.stop();
    let log = fs::read_to_string(env.rule_log_path("tcp")).unwrap();
    assert!(log.contains("protocol=tcp"));
}

#[test]
fn udp_round_trip_multi_client_and_large_datagram_work() {
    let env = TestEnv::new("udp-round-trip");
    let source_port = free_udp_port();
    let backend = UdpSocket::bind(("127.0.0.1", 0)).unwrap();
    backend
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let target_port = backend.local_addr().unwrap().port();

    thread::spawn(move || {
        let mut buf = vec![0_u8; 65_535];
        for _ in 0..4 {
            let (n, peer) = backend.recv_from(&mut buf).unwrap();
            backend.send_to(&buf[..n], peer).unwrap();
        }
    });

    env.run(&[
        "add",
        "udp",
        "--protocol",
        "udp",
        "--source",
        &format!("127.0.0.1:{source_port}"),
        "--target",
        &format!("127.0.0.1:{target_port}"),
    ]);
    env.run(&["enable", "udp"]);
    env.run_daemon();
    wait_for_status(&env, &["udp", "protocol: udp", "status: running"]);

    let source_addr: SocketAddr = format!("127.0.0.1:{source_port}").parse().unwrap();
    assert_eq!(
        udp_exchange(
            &UdpSocket::bind(("127.0.0.1", 0)).unwrap(),
            source_addr,
            b"PING_UDP"
        ),
        b"PING_UDP"
    );

    let client_a = UdpSocket::bind(("127.0.0.1", 0)).unwrap();
    let client_b = UdpSocket::bind(("127.0.0.1", 0)).unwrap();
    client_a.send_to(b"CLIENT_A", source_addr).unwrap();
    client_b.send_to(b"CLIENT_B", source_addr).unwrap();
    assert_eq!(udp_recv(&client_a), b"CLIENT_A");
    assert_eq!(udp_recv(&client_b), b"CLIENT_B");

    let large = vec![b'x'; 4096];
    assert_eq!(
        udp_exchange(
            &UdpSocket::bind(("127.0.0.1", 0)).unwrap(),
            source_addr,
            &large
        ),
        large
    );

    env.stop();
    let log = fs::read_to_string(env.rule_log_path("udp")).unwrap();
    assert!(log.contains("protocol=udp"));
    assert!(log.contains("udp session opened"));
}

#[test]
fn tcp_and_udp_can_share_source_port() {
    let env = TestEnv::new("same-port");
    let source_port = free_tcp_udp_port();

    let tcp_backend = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    tcp_backend.set_nonblocking(true).unwrap();
    let tcp_target_port = tcp_backend.local_addr().unwrap().port();

    let udp_backend = UdpSocket::bind(("127.0.0.1", 0)).unwrap();
    udp_backend
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let udp_target_port = udp_backend.local_addr().unwrap().port();
    thread::spawn(move || {
        let mut buf = [0_u8; 64];
        let (n, peer) = udp_backend.recv_from(&mut buf).unwrap();
        udp_backend.send_to(&buf[..n], peer).unwrap();
    });

    env.run(&[
        "add",
        "tcp_same",
        "--source",
        &format!("127.0.0.1:{source_port}"),
        "--target",
        &format!("127.0.0.1:{tcp_target_port}"),
    ]);
    env.run(&[
        "add",
        "udp_same",
        "--protocol",
        "udp",
        "--source",
        &format!("127.0.0.1:{source_port}"),
        "--target",
        &format!("127.0.0.1:{udp_target_port}"),
    ]);
    env.run(&["enable", "tcp_same"]);
    env.run(&["enable", "udp_same"]);
    env.run_daemon();
    wait_for_status(&env, &["tcp_same", "udp_same", "status: running"]);

    thread::spawn(move || {
        let (mut stream, _) = accept_with_timeout(&tcp_backend);
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut buf = [0_u8; 64];
        let n = stream.read(&mut buf).unwrap();
        stream.write_all(&buf[..n]).unwrap();
    });

    let tcp_addr: SocketAddr = format!("127.0.0.1:{source_port}").parse().unwrap();
    let mut tcp_client = TcpStream::connect_timeout(&tcp_addr, Duration::from_secs(3)).unwrap();
    tcp_client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    tcp_client.write_all(b"TCP").unwrap();
    let mut tcp_reply = [0_u8; 3];
    tcp_client.read_exact(&mut tcp_reply).unwrap();
    assert_eq!(&tcp_reply, b"TCP");

    let udp_client = UdpSocket::bind(("127.0.0.1", 0)).unwrap();
    let source_addr: SocketAddr = format!("127.0.0.1:{source_port}").parse().unwrap();
    assert_eq!(udp_exchange(&udp_client, source_addr, b"UDP"), b"UDP");
}

#[test]
fn udp_bind_conflict_reports_failed() {
    let env = TestEnv::new("udp-bind-conflict");
    let guard = UdpSocket::bind(("127.0.0.1", 0)).unwrap();
    let source_port = guard.local_addr().unwrap().port();
    let target_port = free_udp_port();

    env.run(&[
        "add",
        "udp_conflict",
        "--protocol",
        "udp",
        "--source",
        &format!("127.0.0.1:{source_port}"),
        "--target",
        &format!("127.0.0.1:{target_port}"),
    ]);
    env.run(&["enable", "udp_conflict"]);
    env.run_daemon();

    let status = wait_for_status(&env, &["udp_conflict", "status: failed"]);
    assert!(status.contains("failed to bind"));
}

#[test]
fn udp_unresponsive_target_keeps_rule_running_and_idle_cleanup_logs() {
    let env = TestEnv::new("udp-idle").with_udp_idle_timeout_ms(200);
    let source_port = free_udp_port();
    let target_guard = UdpSocket::bind(("127.0.0.1", 0)).unwrap();
    let target_port = target_guard.local_addr().unwrap().port();

    env.run(&[
        "add",
        "udp_idle",
        "--protocol",
        "udp",
        "--source",
        &format!("127.0.0.1:{source_port}"),
        "--target",
        &format!("127.0.0.1:{target_port}"),
    ]);
    env.run(&["enable", "udp_idle"]);
    env.run_daemon();
    wait_for_status(&env, &["udp_idle", "protocol: udp", "status: running"]);

    let client = UdpSocket::bind(("127.0.0.1", 0)).unwrap();
    client
        .send_to(b"NO_BACKEND", format!("127.0.0.1:{source_port}"))
        .unwrap();

    thread::sleep(Duration::from_millis(900));
    let status = env.stdout(&["status"]);
    assert!(status.contains("status: running"));

    let log = fs::read_to_string(env.rule_log_path("udp_idle")).unwrap();
    assert!(log.contains("udp session closed"));
    assert!(log.contains("idle_timeout"));
}

fn wait_for_status(env: &TestEnv, needles: &[&str]) -> String {
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut last = String::new();

    while Instant::now() < deadline {
        last = env.stdout(&["status"]);
        if needles.iter().all(|needle| last.contains(needle)) {
            return last;
        }
        thread::sleep(Duration::from_millis(100));
    }

    panic!(
        "timed out waiting for {:?}\nlast status:\n{}",
        needles, last
    );
}

fn udp_exchange(socket: &UdpSocket, target: SocketAddr, payload: &[u8]) -> Vec<u8> {
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    socket.send_to(payload, target).unwrap();
    udp_recv(socket)
}

fn udp_recv(socket: &UdpSocket) -> Vec<u8> {
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut buf = vec![0_u8; 65_535];
    let (n, _) = socket.recv_from(&mut buf).unwrap();
    buf.truncate(n);
    buf
}

fn accept_with_timeout(listener: &TcpListener) -> (TcpStream, SocketAddr) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match listener.accept() {
            Ok(result) => return result,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(e) => panic!("tcp backend accept failed: {e}"),
        }
    }
}

fn free_tcp_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn free_udp_port() -> u16 {
    UdpSocket::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn free_tcp_udp_port() -> u16 {
    loop {
        let tcp = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = tcp.local_addr().unwrap().port();
        if let Ok(udp) = UdpSocket::bind(("127.0.0.1", port)) {
            drop(udp);
            drop(tcp);
            return port;
        }
    }
}

#[allow(dead_code)]
fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}
