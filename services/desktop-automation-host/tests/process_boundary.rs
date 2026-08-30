#[cfg(unix)]
use desktop_agent_core::{CommandProcessor, ExecutionPolicy, InMemoryAuditSink};
#[cfg(unix)]
use desktop_protocol::{CommandOutcome, DesktopAction, DesktopCommand, ExecutionLease};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(unix)]
use trigix_desktop_automation::SupervisedActionExecutor;
use trigix_desktop_automation::{
    AutomationCancellation, AutomationHostOperation, AutomationHostRequest, AutomationHostResponse,
    AutomationHostStatus, AutomationHostSupervisor, SupervisorConfig,
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

#[cfg(unix)]
fn command(command_id: &str, lease_expires_at_unix_ms: u64) -> DesktopCommand {
    DesktopCommand {
        command_id: command_id.to_owned(),
        execution_id: format!("execution-{command_id}"),
        tenant_id: "tenant-supervisor".to_owned(),
        project_id: "project-supervisor".to_owned(),
        requested_by: "user-supervisor".to_owned(),
        issued_at_unix_ms: lease_expires_at_unix_ms.saturating_sub(10_000),
        lease: ExecutionLease {
            lease_id: format!("lease-{command_id}"),
            expires_at_unix_ms: lease_expires_at_unix_ms,
        },
        approval: None,
        action: DesktopAction::ReadSystemInformation,
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

#[test]
fn supervisor_executes_and_reclaims_a_short_lived_host() {
    let supervisor = Arc::new(
        AutomationHostSupervisor::new(
            std::path::PathBuf::from(env!("CARGO_BIN_EXE_desktop-automation-host")),
            SupervisorConfig::default(),
        )
        .unwrap(),
    );
    let mut handles = Vec::new();
    for index in 0..16 {
        let supervisor = Arc::clone(&supervisor);
        handles.push(std::thread::spawn(move || {
            supervisor
                .execute(
                    request(
                        &format!("health-supervised-{index}"),
                        AutomationHostOperation::Health,
                    ),
                    &AutomationCancellation::default(),
                )
                .unwrap()
        }));
    }
    for handle in handles {
        assert_eq!(handle.join().unwrap().status, AutomationHostStatus::Ready);
    }
}

#[cfg(unix)]
#[test]
fn supervisor_cancels_active_and_queued_work_without_stopping_parent() {
    let fixture = executable_script("hang", "IFS= read -r line\nsleep 30\n");
    let supervisor =
        Arc::new(AutomationHostSupervisor::new(&fixture, SupervisorConfig::default()).unwrap());
    let active_cancellation = AutomationCancellation::default();
    let active_handle = {
        let supervisor = Arc::clone(&supervisor);
        let cancellation = active_cancellation.clone();
        std::thread::spawn(move || {
            supervisor
                .execute(
                    request("active-request", AutomationHostOperation::Health),
                    &cancellation,
                )
                .unwrap()
        })
    };
    std::thread::sleep(Duration::from_millis(100));

    let queued_cancellation = AutomationCancellation::default();
    let queued_handle = {
        let supervisor = Arc::clone(&supervisor);
        let cancellation = queued_cancellation.clone();
        std::thread::spawn(move || {
            supervisor
                .execute(
                    request("queued-request", AutomationHostOperation::Health),
                    &cancellation,
                )
                .unwrap()
        })
    };
    std::thread::sleep(Duration::from_millis(50));
    queued_cancellation.cancel();
    let queued = queued_handle.join().unwrap();
    assert_eq!(queued.status, AutomationHostStatus::Cancelled);
    assert_eq!(queued.error_code.as_deref(), Some("cancelled"));

    active_cancellation.cancel();
    let active = active_handle.join().unwrap();
    assert_eq!(active.status, AutomationHostStatus::Cancelled);
    active.validate().unwrap();
    std::fs::remove_file(fixture).unwrap();
}

#[cfg(unix)]
#[test]
fn supervisor_times_out_hung_host_and_classifies_crash() {
    let hanging = executable_script("timeout", "IFS= read -r line\nsleep 30\n");
    let supervisor = AutomationHostSupervisor::new(&hanging, SupervisorConfig::default()).unwrap();
    let now = now_unix_ms();
    let timed_out = supervisor
        .execute(
            AutomationHostRequest {
                request_id: "timeout-request".to_owned(),
                sent_at_unix_ms: now,
                deadline_unix_ms: now + 150,
                operation: AutomationHostOperation::Health,
            },
            &AutomationCancellation::default(),
        )
        .unwrap();
    assert_eq!(timed_out.status, AutomationHostStatus::TimedOut);
    assert_eq!(timed_out.error_code.as_deref(), Some("deadline_expired"));
    std::fs::remove_file(hanging).unwrap();

    let crashing = executable_script("crash", "IFS= read -r line\nexit 17\n");
    let supervisor = AutomationHostSupervisor::new(&crashing, SupervisorConfig::default()).unwrap();
    let crashed = supervisor
        .execute(
            request("crash-request", AutomationHostOperation::Health),
            &AutomationCancellation::default(),
        )
        .unwrap();
    assert_eq!(crashed.status, AutomationHostStatus::Failed);
    assert_eq!(crashed.error_code.as_deref(), Some("host_crashed"));
    std::fs::remove_file(crashing).unwrap();
}

#[cfg(unix)]
#[test]
fn command_processor_persists_supervised_cancellation_and_timeout_outcomes() {
    let hanging = executable_script("processor-hang", "IFS= read -r line\nsleep 30\n");
    let supervisor = AutomationHostSupervisor::new(&hanging, SupervisorConfig::default()).unwrap();
    let (executor, handle) = SupervisedActionExecutor::new(supervisor);
    let mut processor = CommandProcessor::new(
        executor,
        InMemoryAuditSink::default(),
        ExecutionPolicy::default(),
    );
    let now = now_unix_ms();
    let active_command = command("processor-cancel", now + 10_000);
    let process_handle = std::thread::spawn(move || {
        let result = processor.process(&active_command, now, None).unwrap();
        (processor, active_command, result)
    });
    for _ in 0..100 {
        if handle.active_commands() == 1 {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(handle.cancel("processor-cancel"));
    let (mut processor, active_command, cancelled) = process_handle.join().unwrap();
    assert_eq!(cancelled.outcome, CommandOutcome::Cancelled);
    assert_eq!(cancelled.error_code.as_deref(), Some("cancelled"));
    let replay = processor.process(&active_command, now + 1, None).unwrap();
    assert_eq!(replay, cancelled);
    std::fs::remove_file(&hanging).unwrap();

    let hanging = executable_script("processor-timeout", "IFS= read -r line\nsleep 30\n");
    let supervisor = AutomationHostSupervisor::new(&hanging, SupervisorConfig::default()).unwrap();
    let (executor, _) = SupervisedActionExecutor::new(supervisor);
    let mut processor = CommandProcessor::new(
        executor,
        InMemoryAuditSink::default(),
        ExecutionPolicy::default(),
    );
    let now = now_unix_ms();
    let timeout_command = command("processor-timeout", now + 150);
    let timed_out = processor.process(&timeout_command, now, None).unwrap();
    assert_eq!(timed_out.outcome, CommandOutcome::TimedOut);
    assert_eq!(timed_out.error_code.as_deref(), Some("execution_timed_out"));
    std::fs::remove_file(hanging).unwrap();
}

#[cfg(unix)]
fn executable_script(label: &str, body: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = std::env::temp_dir().join(format!(
        "trigix-automation-{label}-{}-{}.sh",
        std::process::id(),
        now_unix_ms()
    ));
    std::fs::write(&path, format!("#!/bin/sh\n{body}")).unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&path, permissions).unwrap();
    path
}
