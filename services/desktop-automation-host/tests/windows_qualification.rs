#![cfg(windows)]

use desktop_protocol::{
    DesktopAction, DesktopInspectionRequest, DesktopInspectionResult, ElementSelector,
    KeyboardModifier, PointerButton, WindowSelector,
};
use std::mem;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};
use trigix_desktop_automation::{
    AutomationAdapter, AutomationHostError, WindowsAutomationAdapter, FIXTURE_INPUT_AUTOMATION_ID,
    FIXTURE_PASSWORD_AUTOMATION_ID, FIXTURE_SUBMIT_AUTOMATION_ID, FIXTURE_WINDOW_AUTOMATION_ID,
};
use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessHandleCount};

const ITERATIONS: usize = 50;
const FIXTURE_READY_BUDGET: Duration = Duration::from_secs(10);
const LONG_SESSION_BUDGET: Duration = Duration::from_secs(60);
const P95_ACTION_BUDGET: Duration = Duration::from_secs(2);
const HANDLE_GROWTH_BUDGET: u32 = 16;
const WORKING_SET_GROWTH_BUDGET: usize = 64 * 1024 * 1024;

struct FixtureProcess(Child);

impl FixtureProcess {
    fn spawn() -> Self {
        let executable = std::env::var_os("TRIGIX_WINDOWS_FIXTURE_EXE")
            .map(PathBuf::from)
            .expect("TRIGIX_WINDOWS_FIXTURE_EXE must name the built fixture executable");
        assert!(executable.is_absolute(), "fixture path must be absolute");
        Self(
            Command::new(executable)
                .spawn()
                .expect("launch native Windows fixture"),
        )
    }
}

impl Drop for FixtureProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn native_fixture_actions_meet_reliability_latency_and_resource_budgets() {
    let _fixture = FixtureProcess::spawn();
    let mut adapter = WindowsAutomationAdapter::default();
    let window = WindowSelector {
        executable: None,
        title: None,
        automation_id: Some(FIXTURE_WINDOW_AUTOMATION_ID.to_owned()),
        snapshot_id: None,
    };
    let inspected = wait_for_fixture(&mut adapter, &window);
    assert_eq!(inspected.windows.len(), 1);
    assert_eq!(
        inspected.windows[0].selector.automation_id,
        window.automation_id
    );
    let password = inspected.windows[0]
        .elements
        .iter()
        .find(|element| {
            element.selector.automation_id.as_deref() == Some(FIXTURE_PASSWORD_AUTOMATION_ID)
        })
        .expect("password control is present");
    assert!(password.value.is_none());
    assert!(password.redaction.is_some());

    let focused = adapter
        .execute(&DesktopAction::FocusWindow {
            selector: window.clone(),
        })
        .expect("focus native fixture");
    assert_eq!(focused["selector_strategy"], "automation_id");

    let input = element(&window, FIXTURE_INPUT_AUTOMATION_ID, "edit");
    let submit = element(&window, FIXTURE_SUBMIT_AUTOMATION_ID, "button");
    let password = element(&window, FIXTURE_PASSWORD_AUTOMATION_ID, "edit");
    assert_eq!(
        adapter.execute(&DesktopAction::TypeText {
            selector: password,
            text: "must-not-enter".to_owned(),
        }),
        Err(AutomationHostError::ProtectedControl)
    );

    let pressed = adapter
        .execute(&DesktopAction::PressKey {
            selector: window.clone(),
            key: "tab".to_owned(),
            modifiers: vec![KeyboardModifier::Shift],
        })
        .expect("send bounded key input to focused native fixture");
    assert_eq!(pressed["pressed"], true);

    let pointer = adapter
        .execute(&DesktopAction::PointerClick {
            selector: submit.clone(),
            button: PointerButton::Left,
            click_count: 1,
        })
        .expect("send selector-targeted pointer input to native fixture");
    assert_eq!(pointer["targeting"], "selector_center");

    let handles_before = process_handle_count();
    let working_set_before = process_working_set();
    let session_started = Instant::now();
    let mut latencies = Vec::with_capacity(ITERATIONS * 2);
    for index in 0..ITERATIONS {
        let started = Instant::now();
        let typed = adapter
            .execute(&DesktopAction::TypeText {
                selector: input.clone(),
                text: format!("qualification-{index}"),
            })
            .expect("type through native value adapter");
        latencies.push(started.elapsed());
        assert_eq!(typed["semantic_pattern"], "value");

        let started = Instant::now();
        let clicked = adapter
            .execute(&DesktopAction::ClickElement {
                selector: submit.clone(),
            })
            .expect("invoke native fixture button");
        latencies.push(started.elapsed());
        assert_eq!(clicked["semantic_pattern"], "invoke");
    }
    assert!(session_started.elapsed() <= LONG_SESSION_BUDGET);
    latencies.sort_unstable();
    let p95 = latencies[(latencies.len() * 95).div_ceil(100) - 1];
    assert!(p95 <= P95_ACTION_BUDGET, "p95 action latency was {p95:?}");

    let handles_after = process_handle_count();
    assert!(
        handles_after <= handles_before.saturating_add(HANDLE_GROWTH_BUDGET),
        "process handle count grew from {handles_before} to {handles_after}"
    );
    let working_set_after = process_working_set();
    assert!(
        working_set_after <= working_set_before.saturating_add(WORKING_SET_GROWTH_BUDGET),
        "working set grew from {working_set_before} to {working_set_after} bytes"
    );
    println!(
        "qualification iterations={ITERATIONS} p95_ms={} elapsed_ms={} handles_before={handles_before} handles_after={handles_after} working_set_before={working_set_before} working_set_after={working_set_after}",
        p95.as_millis(),
        session_started.elapsed().as_millis(),
    );
}

fn wait_for_fixture(
    adapter: &mut WindowsAutomationAdapter,
    window: &WindowSelector,
) -> DesktopInspectionResult {
    let started = Instant::now();
    loop {
        let request = DesktopInspectionRequest::bounded(Some(window.clone()));
        if let Ok(value) = adapter.execute(&DesktopAction::InspectTargets {
            request: Box::new(request),
        }) {
            let inspected: DesktopInspectionResult =
                serde_json::from_value(value).expect("decode inspection result");
            if !inspected.windows.is_empty() {
                return inspected;
            }
        }
        assert!(
            started.elapsed() < FIXTURE_READY_BUDGET,
            "native fixture did not become discoverable"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn element(window: &WindowSelector, automation_id: &str, control_type: &str) -> ElementSelector {
    ElementSelector {
        window: window.clone(),
        automation_id: Some(automation_id.to_owned()),
        name: None,
        control_type: Some(control_type.to_owned()),
    }
}

fn process_handle_count() -> u32 {
    let mut count = 0;
    let succeeded = unsafe { GetProcessHandleCount(GetCurrentProcess(), &mut count) };
    assert_ne!(succeeded, 0, "read process handle count");
    count
}

fn process_working_set() -> usize {
    let mut counters: PROCESS_MEMORY_COUNTERS = unsafe { mem::zeroed() };
    counters.cb = mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    let succeeded = unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut counters,
            mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        )
    };
    assert_ne!(succeeded, 0, "read process memory counters");
    counters.WorkingSetSize
}
