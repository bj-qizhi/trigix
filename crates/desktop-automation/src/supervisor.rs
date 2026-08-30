use super::{
    failure_response, AutomationHostError, AutomationHostRequest, AutomationHostResponse,
    AutomationHostStatus, MAX_HOST_MESSAGE_BYTES,
};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard, TryLockError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    pub poll_interval: Duration,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(10),
        }
    }
}

impl SupervisorConfig {
    fn validate(&self) -> Result<(), AutomationHostError> {
        if self.poll_interval.is_zero() || self.poll_interval > Duration::from_secs(1) {
            return Err(AutomationHostError::InvalidRequest(
                "supervisor.poll_interval",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct AutomationCancellation {
    cancelled: Arc<AtomicBool>,
}

impl AutomationCancellation {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

#[derive(Debug)]
pub struct AutomationHostSupervisor {
    executable: PathBuf,
    config: SupervisorConfig,
    execution_gate: Mutex<()>,
}

impl AutomationHostSupervisor {
    pub fn new(
        executable: impl Into<PathBuf>,
        config: SupervisorConfig,
    ) -> Result<Self, AutomationHostError> {
        config.validate()?;
        let executable = executable.into();
        if executable.as_os_str().is_empty() || !Path::new(&executable).is_absolute() {
            return Err(AutomationHostError::InvalidRequest("supervisor.executable"));
        }
        Ok(Self {
            executable,
            config,
            execution_gate: Mutex::new(()),
        })
    }

    pub fn execute(
        &self,
        request: AutomationHostRequest,
        cancellation: &AutomationCancellation,
    ) -> Result<AutomationHostResponse, AutomationHostError> {
        let request_id = request.request_id.clone();
        let _permit = match self.acquire_permit(&request, cancellation)? {
            Permit::Acquired(guard) => guard,
            Permit::Cancelled => return Ok(cancelled_response(request_id)),
            Permit::TimedOut => return Ok(timed_out_response(request_id)),
        };
        if cancellation.is_cancelled() {
            return Ok(cancelled_response(request_id));
        }
        let now = now_unix_ms();
        if now >= request.deadline_unix_ms {
            return Ok(timed_out_response(request_id));
        }
        request.validate(now)?;

        let mut child = Command::new(&self.executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| AutomationHostError::Io(error.to_string()))?;
        let Some(mut stdin) = child.stdin.take() else {
            terminate_child(&mut child);
            return Err(AutomationHostError::Io(
                "host stdin was not captured".to_owned(),
            ));
        };
        let Some(stdout) = child.stdout.take() else {
            terminate_child(&mut child);
            return Err(AutomationHostError::Io(
                "host stdout was not captured".to_owned(),
            ));
        };
        if let Err(error) = write_request(&mut stdin, &request) {
            terminate_child(&mut child);
            return Err(error);
        }
        drop(stdin);

        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let _ = sender.send(read_response(BufReader::new(stdout)));
        });

        loop {
            if cancellation.is_cancelled() {
                terminate_child(&mut child);
                return Ok(cancelled_response(request_id));
            }
            if now_unix_ms() >= request.deadline_unix_ms {
                terminate_child(&mut child);
                return Ok(timed_out_response(request_id));
            }
            match receiver.recv_timeout(self.config.poll_interval) {
                Ok(Ok(response)) => {
                    terminate_child(&mut child);
                    if response.request_id != request_id {
                        return Ok(failure_response(
                            request_id,
                            AutomationHostStatus::Failed,
                            "host_protocol_error",
                            "automation host returned a mismatched request identifier",
                        ));
                    }
                    response.validate()?;
                    return Ok(response);
                }
                Ok(Err(error)) => {
                    terminate_child(&mut child);
                    if error == AutomationHostError::HostCrashed {
                        return Ok(failure_response(
                            request_id,
                            AutomationHostStatus::Failed,
                            "host_crashed",
                            "automation host exited before returning a result",
                        ));
                    }
                    return Ok(failure_response(
                        request_id,
                        AutomationHostStatus::Failed,
                        "host_protocol_error",
                        &error.to_string(),
                    ));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    terminate_child(&mut child);
                    return Ok(failure_response(
                        request_id,
                        AutomationHostStatus::Failed,
                        "host_crashed",
                        "automation host exited before returning a result",
                    ));
                }
            }
        }
    }

    fn acquire_permit<'a>(
        &'a self,
        request: &AutomationHostRequest,
        cancellation: &AutomationCancellation,
    ) -> Result<Permit<'a>, AutomationHostError> {
        loop {
            if cancellation.is_cancelled() {
                return Ok(Permit::Cancelled);
            }
            if now_unix_ms() >= request.deadline_unix_ms {
                return Ok(Permit::TimedOut);
            }
            match self.execution_gate.try_lock() {
                Ok(guard) => return Ok(Permit::Acquired(guard)),
                Err(TryLockError::WouldBlock) => std::thread::sleep(self.config.poll_interval),
                Err(TryLockError::Poisoned(_)) => {
                    return Err(AutomationHostError::Adapter(
                        "automation concurrency state is poisoned".to_owned(),
                    ))
                }
            }
        }
    }
}

enum Permit<'a> {
    Acquired(MutexGuard<'a, ()>),
    Cancelled,
    TimedOut,
}

fn write_request(
    writer: &mut impl Write,
    request: &AutomationHostRequest,
) -> Result<(), AutomationHostError> {
    let encoded =
        serde_json::to_vec(request).map_err(|error| AutomationHostError::Io(error.to_string()))?;
    if encoded.len() + 1 > MAX_HOST_MESSAGE_BYTES as usize {
        return Err(AutomationHostError::InvalidRequest("message_size"));
    }
    writer.write_all(&encoded)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn read_response(
    mut reader: BufReader<impl std::io::Read>,
) -> Result<AutomationHostResponse, AutomationHostError> {
    let mut encoded = Vec::new();
    let bytes = reader
        .by_ref()
        .take(MAX_HOST_MESSAGE_BYTES + 1)
        .read_until(b'\n', &mut encoded)?;
    if bytes == 0 {
        return Err(AutomationHostError::HostCrashed);
    }
    if bytes as u64 > MAX_HOST_MESSAGE_BYTES || encoded.last() != Some(&b'\n') {
        return Err(AutomationHostError::InvalidRequest("message_size"));
    }
    serde_json::from_slice(&encoded).map_err(|_| AutomationHostError::InvalidRequest("json"))
}

fn terminate_child(child: &mut Child) {
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill();
    }
    let _ = child.wait();
}

fn cancelled_response(request_id: String) -> AutomationHostResponse {
    failure_response(
        request_id,
        AutomationHostStatus::Cancelled,
        "cancelled",
        "automation request was cancelled by the parent Device",
    )
}

fn timed_out_response(request_id: String) -> AutomationHostResponse {
    failure_response(
        request_id,
        AutomationHostStatus::TimedOut,
        "deadline_expired",
        "automation request deadline expired in the parent Device",
    )
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(u64::MAX)
}
