use super::*;
use axuielement::ax_action::{AX_PRESS_ACTION, AX_RAISE_ACTION};
use axuielement::ax_attribute::attributes::*;
use axuielement::{is_process_trusted, AXUIElement};
use core_graphics::event::{
    CGEvent, CGEventField, CGEventFlags, CGEventTapLocation, CGEventType, CGMouseButton, KeyCode,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use serde::Deserialize;
use std::collections::{hash_map::DefaultHasher, HashMap};
use std::env;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use sysinfo::System;

const APPLICATION_ALLOWLIST_ENV: &str = "TRIGIX_DESKTOP_APPLICATION_ALLOWLIST";
const AX_IDENTIFIER_ATTRIBUTE: &str = "AXIdentifier";
const AX_SECURE_TEXT_FIELD_SUBROLE: &str = "AXSecureTextField";
const MOUSE_EVENT_CLICK_STATE: CGEventField = 1;

pub fn accessibility_trusted() -> bool {
    is_process_trusted()
}

pub fn request_accessibility() -> bool {
    axuielement::is_process_trusted_with_prompt()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplicationRegistration {
    application_id: String,
    executable_path: String,
}

#[derive(Debug, Default)]
pub struct MacosAutomationAdapter {
    applications: HashMap<String, PathBuf>,
}

#[derive(Clone)]
struct NativeWindow {
    element: AXUIElement,
    inspected: InspectedWindow,
}

#[derive(Clone)]
struct NativeElement {
    element: AXUIElement,
    window: NativeWindow,
    protected: bool,
}

struct NativeWindowEnumeration {
    windows: Vec<NativeWindow>,
    truncated: bool,
}

impl MacosAutomationAdapter {
    pub fn from_environment() -> Result<Self, AutomationHostError> {
        let Some(value) = env::var_os(APPLICATION_ALLOWLIST_ENV) else {
            return Ok(Self::default());
        };
        let registrations: Vec<ApplicationRegistration> = serde_json::from_str(
            value
                .to_str()
                .ok_or(AutomationHostError::InvalidRequest("application_allowlist"))?,
        )
        .map_err(|_| AutomationHostError::InvalidRequest("application_allowlist"))?;
        let mut applications = HashMap::new();
        for registration in registrations {
            desktop_protocol::ApplicationIdentity::new(&registration.application_id).validate()?;
            let path = PathBuf::from(&registration.executable_path);
            if !valid_application_path(&path)
                || registration.executable_path.chars().any(char::is_control)
                || applications
                    .insert(registration.application_id, path)
                    .is_some()
            {
                return Err(AutomationHostError::InvalidRequest("application_allowlist"));
            }
        }
        Ok(Self { applications })
    }

    fn require_accessibility() -> Result<(), AutomationHostError> {
        if is_process_trusted() {
            Ok(())
        } else {
            Err(AutomationHostError::AccessDenied)
        }
    }

    fn launch(
        &self,
        application_id: &desktop_protocol::ApplicationIdentity,
        guard: Option<&AutomationExecutionGuard<'_>>,
    ) -> Result<Value, AutomationHostError> {
        let path = self
            .applications
            .get(&application_id.0)
            .ok_or(AutomationHostError::ApplicationNotAllowed)?;
        if let Some(guard) = guard {
            guard.ensure_active()?;
        }
        let status = Command::new("/usr/bin/open")
            .arg("-a")
            .arg(path)
            .status()
            .map_err(|_| AutomationHostError::LaunchFailed)?;
        if !status.success() {
            return Err(AutomationHostError::LaunchFailed);
        }
        Ok(json!({
            "application_id": application_id.0,
            "launched": true,
        }))
    }

    fn focus(
        &self,
        selector: &WindowSelector,
        guard: Option<&AutomationExecutionGuard<'_>>,
    ) -> Result<Value, AutomationHostError> {
        Self::require_accessibility()?;
        let resolved = resolve_native_window(selector)?;
        verify_snapshot(
            &resolved.target.inspected.selector,
            selector.snapshot_id.as_deref(),
        )?;
        if let Some(guard) = guard {
            guard.ensure_active()?;
        }
        let pid = resolved.target.inspected.process_id;
        let app = AXUIElement::from_pid(pid as i32).ok_or(AutomationHostError::TargetNotFound)?;
        app.set_bool_attribute(AX_FRONTMOST_ATTRIBUTE, true)
            .map_err(map_ax_error)?;
        if let Some(guard) = guard {
            guard.ensure_active()?;
        }
        resolved
            .target
            .element
            .perform_action(AX_RAISE_ACTION)
            .map_err(map_ax_error)?;
        if let Some(guard) = guard {
            guard.ensure_active()?;
        }
        let _ = resolved
            .target
            .element
            .set_bool_attribute(AX_MAIN_ATTRIBUTE, true);
        let _ = resolved
            .target
            .element
            .set_bool_attribute(AX_FOCUSED_ATTRIBUTE, true);
        if app.bool_attribute(AX_FRONTMOST_ATTRIBUTE).ok().flatten() != Some(true) {
            return Err(AutomationHostError::FocusChanged);
        }
        Ok(json!({
            "focused": true,
            "process_id": pid,
            "selector_strategy": resolved.telemetry.strategy,
            "selector_fallback_depth": resolved.telemetry.fallback_depth,
            "selector_fallback_used": resolved.telemetry.fallback_used(),
        }))
    }

    fn click(
        &self,
        selector: &ElementSelector,
        guard: Option<&AutomationExecutionGuard<'_>>,
    ) -> Result<Value, AutomationHostError> {
        Self::require_accessibility()?;
        let resolved = resolve_native_element(selector)?;
        ensure_frontmost(resolved.target.window.inspected.process_id)?;
        let actions = resolved
            .target
            .element
            .action_names()
            .map_err(map_ax_error)?;
        if !actions.iter().any(|action| action == AX_PRESS_ACTION) {
            return Err(AutomationHostError::UnsupportedPattern);
        }
        if let Some(guard) = guard {
            guard.ensure_active()?;
        }
        resolved
            .target
            .element
            .perform_action(AX_PRESS_ACTION)
            .map_err(map_ax_error)?;
        Ok(telemetry_output("clicked", resolved.telemetry, "invoke"))
    }

    fn type_text(
        &self,
        selector: &ElementSelector,
        text: &str,
        guard: Option<&AutomationExecutionGuard<'_>>,
    ) -> Result<Value, AutomationHostError> {
        Self::require_accessibility()?;
        let resolved = resolve_native_element(selector)?;
        if resolved.target.protected {
            return Err(AutomationHostError::ProtectedControl);
        }
        ensure_frontmost(resolved.target.window.inspected.process_id)?;
        if !resolved
            .target
            .element
            .is_attribute_settable(AX_VALUE_ATTRIBUTE)
            .map_err(map_ax_error)?
        {
            return Err(AutomationHostError::UnsupportedPattern);
        }
        if let Some(guard) = guard {
            guard.ensure_active()?;
        }
        resolved
            .target
            .element
            .set_string_attribute(AX_VALUE_ATTRIBUTE, text)
            .map_err(map_ax_error)?;
        if resolved
            .target
            .element
            .string_attribute(AX_VALUE_ATTRIBUTE)
            .map_err(map_ax_error)?
            .as_deref()
            != Some(text)
        {
            return Err(AutomationHostError::PartialEntry);
        }
        Ok(json!({
            "entered": true,
            "characters_entered": text.chars().count(),
            "semantic_pattern": "value",
            "selector_strategy": resolved.telemetry.strategy,
            "selector_fallback_depth": resolved.telemetry.fallback_depth,
            "selector_fallback_used": resolved.telemetry.fallback_used(),
        }))
    }

    fn press_key(
        &self,
        selector: &WindowSelector,
        key: &str,
        modifiers: &[KeyboardModifier],
        guard: Option<&AutomationExecutionGuard<'_>>,
    ) -> Result<Value, AutomationHostError> {
        Self::require_accessibility()?;
        let resolved = resolve_native_window(selector)?;
        verify_snapshot(
            &resolved.target.inspected.selector,
            selector.snapshot_id.as_deref(),
        )?;
        ensure_frontmost(resolved.target.inspected.process_id)?;
        let key_code = virtual_key(key).ok_or(AutomationHostError::UnsupportedPattern)?;
        let source = event_source()?;
        let flags = modifier_flags(modifiers);
        let down = CGEvent::new_keyboard_event(source.clone(), key_code, true)
            .map_err(|_| AutomationHostError::AccessDenied)?;
        let up = CGEvent::new_keyboard_event(source, key_code, false)
            .map_err(|_| AutomationHostError::AccessDenied)?;
        down.set_flags(flags);
        up.set_flags(flags);
        if let Some(guard) = guard {
            guard.ensure_active()?;
        }
        down.post(CGEventTapLocation::HID);
        up.post(CGEventTapLocation::HID);
        Ok(json!({
            "pressed": true,
            "key": key,
            "modifier_count": modifiers.len(),
            "selector_strategy": resolved.telemetry.strategy,
            "selector_fallback_depth": resolved.telemetry.fallback_depth,
            "selector_fallback_used": resolved.telemetry.fallback_used(),
        }))
    }

    fn pointer_click(
        &self,
        selector: &ElementSelector,
        button: PointerButton,
        click_count: u8,
        guard: Option<&AutomationExecutionGuard<'_>>,
    ) -> Result<Value, AutomationHostError> {
        Self::require_accessibility()?;
        let resolved = resolve_native_element(selector)?;
        ensure_frontmost(resolved.target.window.inspected.process_id)?;
        let position = resolved
            .target
            .element
            .point_attribute(AX_POSITION_ATTRIBUTE)
            .map_err(map_ax_error)?
            .ok_or(AutomationHostError::TargetNotFound)?;
        let size = resolved
            .target
            .element
            .size_attribute(AX_SIZE_ATTRIBUTE)
            .map_err(map_ax_error)?
            .ok_or(AutomationHostError::TargetNotFound)?;
        if size.width <= 0.0 || size.height <= 0.0 {
            return Err(AutomationHostError::TargetNotFound);
        }
        let point = CGPoint::new(
            position.x + size.width / 2.0,
            position.y + size.height / 2.0,
        );
        let (mouse_button, down_type, up_type) = match button {
            PointerButton::Left => (
                CGMouseButton::Left,
                CGEventType::LeftMouseDown,
                CGEventType::LeftMouseUp,
            ),
            PointerButton::Right => (
                CGMouseButton::Right,
                CGEventType::RightMouseDown,
                CGEventType::RightMouseUp,
            ),
            PointerButton::Middle => (
                CGMouseButton::Center,
                CGEventType::OtherMouseDown,
                CGEventType::OtherMouseUp,
            ),
        };
        let source = event_source()?;
        for count in 1..=click_count {
            if let Some(guard) = guard {
                guard.ensure_active()?;
            }
            let down = CGEvent::new_mouse_event(source.clone(), down_type, point, mouse_button)
                .map_err(|_| AutomationHostError::AccessDenied)?;
            let up = CGEvent::new_mouse_event(source.clone(), up_type, point, mouse_button)
                .map_err(|_| AutomationHostError::AccessDenied)?;
            down.set_integer_value_field(MOUSE_EVENT_CLICK_STATE, i64::from(count));
            up.set_integer_value_field(MOUSE_EVENT_CLICK_STATE, i64::from(count));
            down.post(CGEventTapLocation::HID);
            up.post(CGEventTapLocation::HID);
        }
        Ok(json!({
            "clicked": true,
            "pointer_button": button,
            "click_count": click_count,
            "targeting": "selector_center",
            "selector_strategy": resolved.telemetry.strategy,
            "selector_fallback_depth": resolved.telemetry.fallback_depth,
            "selector_fallback_used": resolved.telemetry.fallback_used(),
        }))
    }
}

impl AutomationAdapter for MacosAutomationAdapter {
    fn execute(&mut self, action: &DesktopAction) -> Result<Value, AutomationHostError> {
        match action {
            DesktopAction::ReadSystemInformation => Ok(json!({
                "adapter": "macos_accessibility",
                "platform": "macos",
                "accessibility_trusted": is_process_trusted(),
            })),
            DesktopAction::InspectTargets { request } => {
                Self::require_accessibility()?;
                serde_json::to_value(inspect_windows(request)?)
                    .map_err(|error| AutomationHostError::Adapter(error.to_string()))
            }
            DesktopAction::FocusWindow { selector } => self.focus(selector, None),
            DesktopAction::LaunchApplication { application_id } => {
                self.launch(application_id, None)
            }
            DesktopAction::ClickElement { selector } => self.click(selector, None),
            DesktopAction::TypeText { selector, text } => self.type_text(selector, text, None),
            DesktopAction::PressKey {
                selector,
                key,
                modifiers,
            } => self.press_key(selector, key, modifiers, None),
            DesktopAction::PointerClick {
                selector,
                button,
                click_count,
            } => self.pointer_click(selector, *button, *click_count, None),
        }
    }

    fn execute_guarded(
        &mut self,
        action: &DesktopAction,
        guard: &AutomationExecutionGuard<'_>,
    ) -> Result<Value, AutomationHostError> {
        guard.ensure_active()?;
        match action {
            DesktopAction::ReadSystemInformation | DesktopAction::InspectTargets { .. } => {
                self.execute(action)
            }
            DesktopAction::FocusWindow { selector } => self.focus(selector, Some(guard)),
            DesktopAction::LaunchApplication { application_id } => {
                self.launch(application_id, Some(guard))
            }
            DesktopAction::ClickElement { selector } => self.click(selector, Some(guard)),
            DesktopAction::TypeText { selector, text } => {
                self.type_text(selector, text, Some(guard))
            }
            DesktopAction::PressKey {
                selector,
                key,
                modifiers,
            } => self.press_key(selector, key, modifiers, Some(guard)),
            DesktopAction::PointerClick {
                selector,
                button,
                click_count,
            } => self.pointer_click(selector, *button, *click_count, Some(guard)),
        }
    }
}

fn inspect_windows(
    request: &DesktopInspectionRequest,
) -> Result<DesktopInspectionResult, AutomationHostError> {
    let mut enumeration = enumerate_windows(request)?;
    if enumeration.windows.is_empty() {
        return Err(AutomationHostError::TargetNotFound);
    }
    let mut windows = enumeration
        .windows
        .drain(..)
        .map(|window| window.inspected)
        .collect::<Vec<_>>();
    let snapshot_id = snapshot_id(&windows)?;
    if request
        .expected_snapshot_id
        .as_deref()
        .is_some_and(|expected| expected != snapshot_id)
    {
        return Err(AutomationHostError::TargetStale);
    }
    let selector_resolution = request
        .visual_suggestion
        .as_ref()
        .map(|suggestion| {
            confirm_visual_suggestion(&mut windows, suggestion, &snapshot_id, current_unix_ms())
        })
        .transpose()?;
    attach_snapshot(&mut windows, &snapshot_id);
    bound_result(
        DesktopInspectionResult {
            snapshot_id,
            windows,
            truncated: enumeration.truncated,
            selector_resolution,
        },
        request,
    )
}

fn enumerate_windows(
    request: &DesktopInspectionRequest,
) -> Result<NativeWindowEnumeration, AutomationHostError> {
    let started = Instant::now();
    let deadline = Duration::from_millis(u64::from(request.max_duration_ms));
    let system = System::new_all();
    let mut windows = Vec::new();
    let mut element_count = 0usize;
    for (pid, process) in system.processes() {
        if started.elapsed() >= deadline || windows.len() >= request.max_windows as usize {
            break;
        }
        let Ok(pid_i32) = i32::try_from(pid.as_u32()) else {
            continue;
        };
        let Some(application) = AXUIElement::from_pid(pid_i32) else {
            continue;
        };
        let _ = application.set_timeout(0.5);
        let Ok(ax_windows) = application.element_array_attribute(AX_WINDOWS_ATTRIBUTE) else {
            continue;
        };
        for (index, window) in ax_windows.into_iter().enumerate() {
            if started.elapsed() >= deadline || windows.len() >= request.max_windows as usize {
                break;
            }
            let title = window.string_attribute(AX_TITLE_ATTRIBUTE).ok().flatten();
            let sensitive_title = title.as_deref().is_some_and(is_credential_text);
            let executable = process
                .exe()
                .and_then(Path::file_name)
                .map(|name| name.to_string_lossy().into_owned())
                .or_else(|| Some(process.name().to_string_lossy().into_owned()));
            let automation_id = window
                .string_attribute(AX_IDENTIFIER_ATTRIBUTE)
                .ok()
                .flatten()
                .or_else(|| Some(format!("pid-{pid_i32}-window-{index}")));
            let selector = WindowSelector {
                executable,
                title: if sensitive_title { None } else { title },
                automation_id,
                snapshot_id: None,
            };
            if request
                .window
                .as_ref()
                .is_some_and(|query| !window_matches(&selector, query))
            {
                continue;
            }
            let remaining = (request.max_elements as usize).saturating_sub(element_count);
            let mut elements = Vec::new();
            collect_elements(
                &window,
                &selector,
                1,
                request.max_depth,
                remaining,
                started,
                deadline,
                &mut elements,
            );
            element_count += elements.len();
            windows.push(NativeWindow {
                element: window,
                inspected: InspectedWindow {
                    selector,
                    process_id: pid.as_u32(),
                    title_policy: if sensitive_title {
                        WindowTitlePolicy::Redacted
                    } else {
                        WindowTitlePolicy::Exact
                    },
                    elements,
                },
            });
        }
    }
    Ok(NativeWindowEnumeration {
        truncated: started.elapsed() >= deadline
            || windows.len() >= request.max_windows as usize
            || element_count >= request.max_elements as usize,
        windows,
    })
}

#[allow(clippy::too_many_arguments)]
fn collect_elements(
    element: &AXUIElement,
    window: &WindowSelector,
    depth: u8,
    max_depth: u8,
    max_elements: usize,
    started: Instant,
    deadline: Duration,
    output: &mut Vec<InspectedElement>,
) {
    if depth > max_depth || output.len() >= max_elements || started.elapsed() >= deadline {
        return;
    }
    let Ok(children) = element.children() else {
        return;
    };
    for child in children {
        if output.len() >= max_elements || started.elapsed() >= deadline {
            return;
        }
        let role = child
            .string_attribute(AX_ROLE_ATTRIBUTE)
            .ok()
            .flatten()
            .unwrap_or_else(|| "AXUnknown".to_owned());
        let control_type = normalize_role(&role).to_owned();
        let name = child
            .string_attribute(AX_TITLE_ATTRIBUTE)
            .ok()
            .flatten()
            .or_else(|| {
                child
                    .string_attribute(AX_DESCRIPTION_ATTRIBUTE)
                    .ok()
                    .flatten()
            });
        let automation_id = child
            .string_attribute(AX_IDENTIFIER_ATTRIBUTE)
            .ok()
            .flatten();
        let subrole = child.string_attribute(AX_SUBROLE_ATTRIBUTE).ok().flatten();
        let protected = subrole.as_deref() == Some(AX_SECURE_TEXT_FIELD_SUBROLE)
            || name.as_deref().is_some_and(is_credential_text)
            || automation_id.as_deref().is_some_and(is_credential_text);
        let action_names = child.action_names().unwrap_or_default();
        let mut patterns = Vec::new();
        if action_names.iter().any(|action| action == AX_PRESS_ACTION) {
            patterns.push(AutomationPattern::Invoke);
        }
        if child
            .is_attribute_settable(AX_VALUE_ATTRIBUTE)
            .unwrap_or(false)
        {
            patterns.push(AutomationPattern::Value);
        }
        if matches!(control_type.as_str(), "edit" | "text") {
            patterns.push(AutomationPattern::Text);
        }
        let value = if protected {
            None
        } else {
            child
                .string_attribute(AX_VALUE_ATTRIBUTE)
                .ok()
                .flatten()
                .filter(|value| value.len() <= 2_048)
        };
        let oversized = !protected
            && child
                .string_attribute(AX_VALUE_ATTRIBUTE)
                .ok()
                .flatten()
                .is_some_and(|value| value.len() > 2_048);
        output.push(InspectedElement {
            selector: ElementSelector {
                window: window.clone(),
                automation_id,
                name,
                control_type: Some(control_type),
            },
            depth,
            supported_patterns: patterns,
            value,
            redaction: if protected {
                Some(RedactionReason::Password)
            } else if oversized {
                Some(RedactionReason::Oversized)
            } else {
                None
            },
        });
        collect_elements(
            &child,
            window,
            depth.saturating_add(1),
            max_depth,
            max_elements,
            started,
            deadline,
            output,
        );
    }
}

fn resolve_native_window(
    selector: &WindowSelector,
) -> Result<ResolvedTarget<NativeWindow>, AutomationHostError> {
    let request = DesktopInspectionRequest::bounded(None);
    let mut windows = enumerate_windows(&request)?.windows;
    let inspected = windows
        .iter()
        .map(|window| window.inspected.clone())
        .collect::<Vec<_>>();
    let current_snapshot = snapshot_id(&inspected)?;
    for window in &mut windows {
        window.inspected.selector.snapshot_id = Some(current_snapshot.clone());
        for element in &mut window.inspected.elements {
            element.selector.window.snapshot_id = Some(current_snapshot.clone());
        }
    }
    let candidates = windows
        .into_iter()
        .map(|window| (window.inspected.selector.clone(), window))
        .collect::<Vec<_>>();
    resolve_window_from_candidates(selector, &candidates)
}

fn resolve_native_element(
    selector: &ElementSelector,
) -> Result<ResolvedTarget<NativeElement>, AutomationHostError> {
    let resolved_window = resolve_native_window(&selector.window)?;
    verify_snapshot(
        &resolved_window.target.inspected.selector,
        selector.window.snapshot_id.as_deref(),
    )?;
    let native_window = resolved_window.target;
    let mut candidates = Vec::new();
    collect_native_candidates(
        &native_window.element,
        &native_window,
        &mut candidates,
        1,
        desktop_protocol::MAX_INSPECTION_DEPTH,
    );
    resolve_element_from_candidates(
        selector,
        &candidates,
        resolved_window.telemetry.fallback_depth,
    )
}

fn collect_native_candidates(
    element: &AXUIElement,
    window: &NativeWindow,
    output: &mut Vec<(ElementSelector, NativeElement)>,
    depth: u8,
    max_depth: u8,
) {
    if depth > max_depth {
        return;
    }
    let Ok(children) = element.children() else {
        return;
    };
    for child in children {
        let name = child
            .string_attribute(AX_TITLE_ATTRIBUTE)
            .ok()
            .flatten()
            .or_else(|| {
                child
                    .string_attribute(AX_DESCRIPTION_ATTRIBUTE)
                    .ok()
                    .flatten()
            });
        let automation_id = child
            .string_attribute(AX_IDENTIFIER_ATTRIBUTE)
            .ok()
            .flatten();
        let role = child
            .string_attribute(AX_ROLE_ATTRIBUTE)
            .ok()
            .flatten()
            .unwrap_or_else(|| "AXUnknown".to_owned());
        let subrole = child.string_attribute(AX_SUBROLE_ATTRIBUTE).ok().flatten();
        let protected = subrole.as_deref() == Some(AX_SECURE_TEXT_FIELD_SUBROLE)
            || name.as_deref().is_some_and(is_credential_text)
            || automation_id.as_deref().is_some_and(is_credential_text);
        let candidate = ElementSelector {
            window: window.inspected.selector.clone(),
            automation_id,
            name,
            control_type: Some(normalize_role(&role).to_owned()),
        };
        output.push((
            candidate,
            NativeElement {
                element: child.clone(),
                window: window.clone(),
                protected,
            },
        ));
        collect_native_candidates(&child, window, output, depth.saturating_add(1), max_depth);
    }
}

fn valid_application_path(path: &Path) -> bool {
    path.is_absolute()
        && path
            .components()
            .all(|component| !matches!(component, std::path::Component::ParentDir))
        && (path.extension().is_some_and(|extension| extension == "app") || path.is_file())
}

fn verify_snapshot(
    candidate: &WindowSelector,
    expected: Option<&str>,
) -> Result<(), AutomationHostError> {
    if expected.is_none() || candidate.snapshot_id.as_deref() == expected {
        Ok(())
    } else {
        Err(AutomationHostError::TargetStale)
    }
}

fn ensure_frontmost(pid: u32) -> Result<(), AutomationHostError> {
    let app = AXUIElement::from_pid(pid as i32).ok_or(AutomationHostError::TargetNotFound)?;
    if app.bool_attribute(AX_FRONTMOST_ATTRIBUTE).ok().flatten() == Some(true) {
        Ok(())
    } else {
        Err(AutomationHostError::FocusChanged)
    }
}

fn snapshot_id(windows: &[InspectedWindow]) -> Result<String, AutomationHostError> {
    let encoded = serde_json::to_vec(windows)
        .map_err(|error| AutomationHostError::Adapter(error.to_string()))?;
    let mut hasher = DefaultHasher::new();
    encoded.hash(&mut hasher);
    Ok(format!("macos-{:016x}", hasher.finish()))
}

fn normalize_role(role: &str) -> &str {
    match role {
        "AXButton" => "button",
        "AXTextField" | "AXTextArea" | "AXComboBox" => "edit",
        "AXStaticText" => "text",
        "AXCheckBox" => "checkbox",
        "AXRadioButton" => "radio",
        "AXPopUpButton" => "combobox",
        "AXTable" => "table",
        "AXRow" => "row",
        "AXCell" => "cell",
        "AXLink" => "link",
        "AXMenuItem" => "menu_item",
        "AXSlider" => "slider",
        _ => role.strip_prefix("AX").unwrap_or(role),
    }
}

fn is_credential_text(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    [
        "password",
        "passwd",
        "secret",
        "token",
        "credential",
        "api_key",
        "apikey",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
        || value.contains("密码")
        || value.contains("密钥")
        || value.contains("凭据")
}

fn map_ax_error(error: axuielement::AXError) -> AutomationHostError {
    AutomationHostError::Adapter(error.to_string())
}

fn telemetry_output(key: &str, telemetry: ResolutionTelemetry, pattern: &str) -> Value {
    let mut output = json!({
        "semantic_pattern": pattern,
        "selector_strategy": telemetry.strategy,
        "selector_fallback_depth": telemetry.fallback_depth,
        "selector_fallback_used": telemetry.fallback_used(),
    });
    output
        .as_object_mut()
        .expect("telemetry output is an object")
        .insert(key.to_owned(), Value::Bool(true));
    output
}

fn event_source() -> Result<CGEventSource, AutomationHostError> {
    CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| AutomationHostError::AccessDenied)
}

fn modifier_flags(modifiers: &[KeyboardModifier]) -> CGEventFlags {
    modifiers
        .iter()
        .fold(CGEventFlags::empty(), |flags, modifier| {
            flags
                | match modifier {
                    KeyboardModifier::Control => CGEventFlags::CGEventFlagControl,
                    KeyboardModifier::Alt => CGEventFlags::CGEventFlagAlternate,
                    KeyboardModifier::Shift => CGEventFlags::CGEventFlagShift,
                    KeyboardModifier::Meta => CGEventFlags::CGEventFlagCommand,
                }
        })
}

fn virtual_key(key: &str) -> Option<u16> {
    match key {
        "a" => Some(KeyCode::ANSI_A),
        "b" => Some(KeyCode::ANSI_B),
        "c" => Some(KeyCode::ANSI_C),
        "d" => Some(KeyCode::ANSI_D),
        "e" => Some(KeyCode::ANSI_E),
        "f" => Some(KeyCode::ANSI_F),
        "g" => Some(KeyCode::ANSI_G),
        "h" => Some(KeyCode::ANSI_H),
        "i" => Some(KeyCode::ANSI_I),
        "j" => Some(KeyCode::ANSI_J),
        "k" => Some(KeyCode::ANSI_K),
        "l" => Some(KeyCode::ANSI_L),
        "m" => Some(KeyCode::ANSI_M),
        "n" => Some(KeyCode::ANSI_N),
        "o" => Some(KeyCode::ANSI_O),
        "p" => Some(KeyCode::ANSI_P),
        "q" => Some(KeyCode::ANSI_Q),
        "r" => Some(KeyCode::ANSI_R),
        "s" => Some(KeyCode::ANSI_S),
        "t" => Some(KeyCode::ANSI_T),
        "u" => Some(KeyCode::ANSI_U),
        "v" => Some(KeyCode::ANSI_V),
        "w" => Some(KeyCode::ANSI_W),
        "x" => Some(KeyCode::ANSI_X),
        "y" => Some(KeyCode::ANSI_Y),
        "z" => Some(KeyCode::ANSI_Z),
        "0" => Some(KeyCode::ANSI_0),
        "1" => Some(KeyCode::ANSI_1),
        "2" => Some(KeyCode::ANSI_2),
        "3" => Some(KeyCode::ANSI_3),
        "4" => Some(KeyCode::ANSI_4),
        "5" => Some(KeyCode::ANSI_5),
        "6" => Some(KeyCode::ANSI_6),
        "7" => Some(KeyCode::ANSI_7),
        "8" => Some(KeyCode::ANSI_8),
        "9" => Some(KeyCode::ANSI_9),
        "enter" => Some(KeyCode::RETURN),
        "escape" => Some(KeyCode::ESCAPE),
        "tab" => Some(KeyCode::TAB),
        "backspace" => Some(KeyCode::DELETE),
        "delete" => Some(KeyCode::FORWARD_DELETE),
        "space" => Some(KeyCode::SPACE),
        "home" => Some(KeyCode::HOME),
        "end" => Some(KeyCode::END),
        "page_up" => Some(KeyCode::PAGE_UP),
        "page_down" => Some(KeyCode::PAGE_DOWN),
        "arrow_up" => Some(KeyCode::UP_ARROW),
        "arrow_down" => Some(KeyCode::DOWN_ARROW),
        "arrow_left" => Some(KeyCode::LEFT_ARROW),
        "arrow_right" => Some(KeyCode::RIGHT_ARROW),
        "f1" => Some(KeyCode::F1),
        "f2" => Some(KeyCode::F2),
        "f3" => Some(KeyCode::F3),
        "f4" => Some(KeyCode::F4),
        "f5" => Some(KeyCode::F5),
        "f6" => Some(KeyCode::F6),
        "f7" => Some(KeyCode::F7),
        "f8" => Some(KeyCode::F8),
        "f9" => Some(KeyCode::F9),
        "f10" => Some(KeyCode::F10),
        "f11" => Some(KeyCode::F11),
        "f12" => Some(KeyCode::F12),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_markers_are_redacted() {
        assert!(is_credential_text("API token"));
        assert!(is_credential_text("密码"));
        assert!(!is_credential_text("Search"));
    }

    #[test]
    fn application_paths_must_be_absolute_and_non_traversing() {
        assert!(valid_application_path(Path::new(
            "/Applications/Safari.app"
        )));
        assert!(!valid_application_path(Path::new("Safari.app")));
        assert!(!valid_application_path(Path::new(
            "/Applications/../tmp/App.app"
        )));
    }
}
