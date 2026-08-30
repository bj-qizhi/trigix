#[cfg(target_os = "windows")]
fn main() {
    trigix_desktop_shell::run();
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("Trigix Desktop is currently supported on Windows only");
}
