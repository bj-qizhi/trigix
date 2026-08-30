fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "shell_status",
            "request_automation_stop",
            "pairing_status",
            "start_device_pairing",
            "complete_device_pairing",
            "forget_device_pairing",
        ]),
    ))
    .expect("failed to build the Trigix Desktop shell");
}
