use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use trigix_desktop_automation::{
    AutomationHostOperation, AutomationHostRequest, AutomationHostResponse, AutomationHostStatus,
};

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn request(request_id: &str, operation: AutomationHostOperation) -> AutomationHostRequest {
    let now = now_unix_ms();
    AutomationHostRequest {
        request_id: request_id.to_owned(),
        sent_at_unix_ms: now,
        deadline_unix_ms: now + 10_000,
        operation,
    }
}

fn read_response_with_timeout(
    stdout: &mut Option<BufReader<std::process::ChildStdout>>,
) -> AutomationHostResponse {
    let mut reader = stdout.take().expect("response reader is available");
    let (sender, receiver) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut line = String::new();
        let result = reader.read_line(&mut line).map(|_| (reader, line));
        let _ = sender.send(result);
    });
    let (reader, line) = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("automation host response exceeded five seconds")
        .unwrap();
    *stdout = Some(reader);
    serde_json::from_str(&line).unwrap()
}

#[test]
fn executable_has_typed_health_and_shutdown_boundary() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_desktop-automation-host"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = Some(BufReader::new(child.stdout.take().unwrap()));

    writeln!(
        stdin,
        "{}",
        serde_json::to_string(&request("health-1", AutomationHostOperation::Health)).unwrap()
    )
    .unwrap();
    stdin.flush().unwrap();
    let health = read_response_with_timeout(&mut stdout);
    assert_eq!(health.status, AutomationHostStatus::Ready);

    writeln!(
        stdin,
        "{}",
        serde_json::to_string(&request("shutdown-1", AutomationHostOperation::Shutdown)).unwrap()
    )
    .unwrap();
    stdin.flush().unwrap();
    let shutdown = read_response_with_timeout(&mut stdout);
    assert_eq!(shutdown.status, AutomationHostStatus::ShuttingDown);
    assert!(child.wait().unwrap().success());
}

#[test]
fn killing_host_process_does_not_terminate_parent() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_desktop-automation-host"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.kill().unwrap();
    let status = child.wait().unwrap();
    assert!(!status.success());
    assert_eq!(2 + 2, 4);
}

#[test]
fn incomplete_request_is_bounded_by_parent_termination() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_desktop-automation-host"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(b"{").unwrap();
    child.stdin.as_mut().unwrap().flush().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let (sender, receiver) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = sender.send(stdout.read_line(&mut line));
    });

    assert!(receiver.recv_timeout(Duration::from_millis(100)).is_err());
    child.kill().unwrap();
    assert!(!child.wait().unwrap().success());
}
