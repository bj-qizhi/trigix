#[cfg(any(target_os = "windows", target_os = "macos"))]
fn main() {
    trigix_desktop_shell::run();
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn main() {
    eprintln!("Trigix Desktop is supported on Windows and macOS");
}
