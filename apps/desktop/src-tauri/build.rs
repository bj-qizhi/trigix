fn main() {
    if !matches!(
        std::env::var("CARGO_CFG_TARGET_OS").as_deref(),
        Ok("windows" | "macos")
    ) {
        return;
    }
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "shell_status",
            "automation_permission_status",
            "request_automation_permission",
            "request_automation_stop",
            "pairing_status",
            "start_device_pairing",
            "complete_device_pairing",
            "forget_device_pairing",
            "bootstrap_realtime_voice",
            "accept_final_voice_transcript",
            "record_voice_telemetry",
            "confirm_realtime_voice_connected",
            "confirm_avatar_renderer_qualified",
        ]),
    ))
    .expect("failed to build the Trigix Desktop shell");
}
