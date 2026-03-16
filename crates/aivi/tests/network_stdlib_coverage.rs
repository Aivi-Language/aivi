use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use aivi::{
    check_modules, desugar_modules, elaborate_expected_coercions, load_modules_from_paths,
    run_test_suite,
};
use tokio_tungstenite::tungstenite::{connect, Message};

#[path = "test_support.rs"]
mod test_support;

const FILE_TIMEOUT_SECS: u64 = 25;

fn network_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("network stdlib test lock")
}

fn run_test_suite_with_timeout(
    program: aivi::HirProgram,
    test_entries: &[(String, String)],
    modules: &[aivi::surface::Module],
    display_name: &str,
    timeout_secs: u64,
) -> Option<Result<aivi::TestReport, aivi::AiviError>> {
    let test_entries = test_entries.to_vec();
    let modules = modules.to_vec();
    let done = Arc::new(AtomicBool::new(false));
    let done2 = done.clone();

    let handle = std::thread::Builder::new()
        .name(format!("test-{display_name}"))
        .stack_size(256 * 1024 * 1024)
        .spawn(move || {
            let result = catch_unwind(AssertUnwindSafe(|| {
                run_test_suite(program, &test_entries, &modules, false, None)
            }));
            done2.store(true, Ordering::Release);
            result
        })
        .ok()?;

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    while !done.load(Ordering::Acquire) {
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let result = handle.join().ok()?;
    match result {
        Ok(report) => Some(report),
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

fn run_stdlib_file(path: &Path) -> aivi::TestReport {
    let mut modules = load_modules_from_paths(&[path.to_path_buf()])
        .unwrap_or_else(|e| panic!("load_modules_from_paths({}): {e}", path.display()));

    let mut diags = check_modules(&modules);
    if !test_support::file_diagnostics_have_non_embedded_errors(&diags) {
        diags.extend(elaborate_expected_coercions(&mut modules));
    }
    diags.retain(|d| !d.path.starts_with("<embedded:"));
    assert!(
        !test_support::file_diagnostics_have_non_embedded_errors(&diags),
        "type errors in {}: {diags:?}",
        path.display()
    );

    let tests = test_support::collect_test_entries(&modules);
    assert!(
        !tests.is_empty(),
        "no @test definitions found in {}",
        path.display()
    );

    let program = desugar_modules(&modules);
    let display = path.display().to_string();
    let result =
        run_test_suite_with_timeout(program, &tests, &modules, &display, FILE_TIMEOUT_SECS)
            .unwrap_or_else(|| panic!("timeout running stdlib tests in {}", path.display()));
    result.unwrap_or_else(|e| panic!("run_test_suite({}): {e}", path.display()))
}

fn reserve_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let port = listener.local_addr().expect("listener addr").port();
    drop(listener);
    port
}

fn wait_for_accept(listener: TcpListener, timeout: Duration) -> TcpStream {
    listener
        .set_nonblocking(true)
        .expect("set listener nonblocking");
    let deadline = Instant::now() + timeout;
    loop {
        match listener.accept() {
            Ok((stream, _)) => return stream,
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    panic!("timed out waiting for tcp accept");
                }
                thread::sleep(Duration::from_millis(25));
            }
            Err(err) => panic!("accept failed: {err}"),
        }
    }
}

fn wait_for_tcp_connect(port: u16, timeout: Duration) -> TcpStream {
    let deadline = Instant::now() + timeout;
    loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(stream) => return stream,
            Err(err) => {
                if Instant::now() >= deadline {
                    panic!("timed out waiting to connect to 127.0.0.1:{port}: {err}");
                }
                thread::sleep(Duration::from_millis(25));
            }
        }
    }
}

struct EnvGuard {
    saved: Vec<(String, Option<String>)>,
}

impl EnvGuard {
    fn set(vars: &[(&str, String)]) -> Self {
        let saved = vars
            .iter()
            .map(|(key, value)| {
                let key = (*key).to_string();
                let old = std::env::var(&key).ok();
                std::env::set_var(&key, value);
                (key, old)
            })
            .collect();
        Self { saved }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.saved.drain(..) {
            match value {
                Some(old) => std::env::set_var(&key, old),
                None => std::env::remove_var(&key),
            }
        }
    }
}

#[test]
fn sockets_stdlib_exercises_real_connection_lifecycle() {
    let _guard = network_test_lock();
    let root = test_support::workspace_root();
    std::env::set_current_dir(&root).expect("set cwd");
    let path = root.join("integration-tests/stdlib/aivi/network/sockets.aivi");

    let client_listener = TcpListener::bind("127.0.0.1:0").expect("bind socket peer");
    let client_port = client_listener.local_addr().expect("peer addr").port();
    let server_port = reserve_port();

    let client_thread = thread::spawn(move || {
        let mut stream = wait_for_accept(client_listener, Duration::from_secs(5));
        let mut request = [0u8; 4];
        stream
            .read_exact(&mut request)
            .expect("read client request");
        assert_eq!(&request, b"ping");
        stream.write_all(b"pong").expect("write client response");
    });

    let server_thread = thread::spawn(move || {
        let mut stream = wait_for_tcp_connect(server_port, Duration::from_secs(5));
        stream.write_all(b"hello").expect("write server request");
        let mut response = [0u8; 5];
        stream
            .read_exact(&mut response)
            .expect("read server response");
        assert_eq!(&response, b"world");
    });

    let _env = EnvGuard::set(&[
        ("AIVI_TEST_SOCKET_SERVER_PORT", client_port.to_string()),
        ("AIVI_TEST_SOCKET_LISTEN_PORT", server_port.to_string()),
    ]);

    let report = run_stdlib_file(&path);
    assert_eq!(report.failed, 0, "unexpected test failures: {report:#?}");

    client_thread.join().expect("client peer thread");
    server_thread.join().expect("server peer thread");
}

#[test]
fn http_server_stdlib_exercises_http_and_websocket_flow() {
    let _guard = network_test_lock();
    let root = test_support::workspace_root();
    std::env::set_current_dir(&root).expect("set cwd");
    let path = root.join("integration-tests/stdlib/aivi/network/http_server.aivi");

    let http_port = reserve_port();
    let ws_port = reserve_port();

    let ws_thread = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match connect(format!("ws://127.0.0.1:{ws_port}/ws")) {
                Ok((mut socket, _)) => {
                    socket
                        .send(Message::Text("hello websocket".into()))
                        .expect("send websocket text");
                    let echo = socket.read().expect("read websocket echo");
                    assert!(matches!(echo, Message::Text(text) if text == "hello websocket"));
                    let close = socket.read().expect("read websocket close");
                    assert!(matches!(close, Message::Close(_)));
                    return;
                }
                Err(err) => {
                    if Instant::now() >= deadline {
                        panic!("timed out waiting for websocket server: {err}");
                    }
                    thread::sleep(Duration::from_millis(25));
                }
            }
        }
    });

    let _env = EnvGuard::set(&[
        ("AIVI_TEST_HTTP_SERVER_PORT", http_port.to_string()),
        ("AIVI_TEST_HTTP_WS_PORT", ws_port.to_string()),
    ]);

    let report = run_stdlib_file(&path);
    assert_eq!(report.failed, 0, "unexpected test failures: {report:#?}");

    ws_thread.join().expect("websocket client thread");
}
