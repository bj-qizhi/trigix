#[cfg(not(windows))]
fn main() {
    let descriptor = trigix_desktop_automation::AutomationFixtureDescriptor::default();
    println!("{}", descriptor.application_id);
    println!("{}", descriptor.window_automation_id);
    println!("{}", descriptor.input_automation_id);
    println!("{}", descriptor.submit_automation_id);
    println!("{}", descriptor.password_automation_id);
}

#[cfg(windows)]
fn main() {
    windows_fixture::run();
}

#[cfg(windows)]
mod windows_fixture {
    use std::mem;
    use std::ptr;
    use trigix_desktop_automation::FIXTURE_WINDOW_AUTOMATION_ID;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, LoadCursorW,
        PostQuitMessage, RegisterClassW, ShowWindow, TranslateMessage, BS_DEFPUSHBUTTON,
        CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, ES_AUTOHSCROLL, ES_PASSWORD, HMENU, IDC_ARROW, MSG,
        SW_SHOW, WM_DESTROY, WNDCLASSW, WS_CHILD, WS_OVERLAPPEDWINDOW, WS_TABSTOP, WS_VISIBLE,
    };

    pub fn run() {
        unsafe {
            let instance = GetModuleHandleW(ptr::null());
            let class_name = wide(FIXTURE_WINDOW_AUTOMATION_ID);
            let window_class = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
                lpszClassName: class_name.as_ptr(),
                ..mem::zeroed()
            };
            if RegisterClassW(&window_class) == 0 {
                panic!("failed to register fixture window class");
            }
            let title = wide("Trigix Automation Fixture");
            let window = CreateWindowExW(
                0,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                720,
                480,
                ptr::null_mut(),
                ptr::null_mut(),
                instance,
                ptr::null_mut(),
            );
            if window.is_null() {
                panic!("failed to create fixture window");
            }
            create_control(
                window,
                instance,
                "EDIT",
                "",
                24,
                36,
                420,
                28,
                1001,
                ES_AUTOHSCROLL,
            );
            create_control(
                window,
                instance,
                "BUTTON",
                "Submit",
                460,
                36,
                120,
                28,
                1002,
                BS_DEFPUSHBUTTON,
            );
            create_control(
                window,
                instance,
                "EDIT",
                "",
                24,
                84,
                420,
                28,
                1003,
                ES_AUTOHSCROLL | ES_PASSWORD,
            );
            ShowWindow(window, SW_SHOW);

            let mut message: MSG = mem::zeroed();
            while GetMessageW(&mut message, ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn create_control(
        parent: HWND,
        instance: *mut core::ffi::c_void,
        class_name: &str,
        text: &str,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        control_id: isize,
        control_style: i32,
    ) {
        let class_name = wide(class_name);
        let text = wide(text);
        let control = CreateWindowExW(
            0,
            class_name.as_ptr(),
            text.as_ptr(),
            (WS_CHILD | WS_VISIBLE | WS_TABSTOP) | control_style as u32,
            x,
            y,
            width,
            height,
            parent,
            control_id as HMENU,
            instance,
            ptr::null_mut(),
        );
        if control.is_null() {
            panic!("failed to create fixture control {control_id}");
        }
    }

    unsafe extern "system" fn window_proc(
        window: HWND,
        message: u32,
        word: WPARAM,
        long: LPARAM,
    ) -> LRESULT {
        if message == WM_DESTROY {
            PostQuitMessage(0);
            0
        } else {
            DefWindowProcW(window, message, word, long)
        }
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn qualification_signing_is_ephemeral_non_exportable_and_blocking() {
        let script = include_str!("../qualify-signature.ps1");
        assert!(script.contains("CN=Trigix Automation Qualification Fixture"));
        assert!(script.contains("-KeyExportPolicy NonExportable"));
        assert!(script.contains("Set-AuthenticodeSignature"));
        assert!(script.contains("Get-AuthenticodeSignature"));
        assert!(script.contains("signature_hash_algorithm = \"sha256\""));
        assert!(script.contains("Remove-QualificationCertificate"));
        assert!(
            script.contains("StateDirectory must be a dedicated trigix-fixture-signing directory")
        );
        assert!(!script.contains("PFX_PASSWORD"));
        assert!(!script.contains("TimestampServer"));

        let workflow = include_str!("../../../.github/workflows/ci.yml");
        let signing = workflow
            .find("Sign and verify deterministic fixture")
            .unwrap();
        let qualification = workflow.find("Run native adapter qualification").unwrap();
        let evidence = workflow.find("Upload fixture signature evidence").unwrap();
        let cleanup = workflow.find("Remove fixture signing state").unwrap();
        assert!(signing < qualification);
        assert!(qualification < evidence);
        assert!(evidence < cleanup);
        assert!(workflow[qualification..evidence].contains("-Action Verify"));
        assert!(workflow[evidence..cleanup].contains("if: always()"));
        assert!(workflow[cleanup..].contains("if: always()"));
    }
}
