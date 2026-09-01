use std::io::{self, BufReader};
use std::time::{SystemTime, UNIX_EPOCH};
use trigix_desktop_automation::run_host;

#[cfg(not(any(windows, target_os = "macos")))]
use trigix_desktop_automation::FixtureAutomationAdapter as PlatformAutomationAdapter;
#[cfg(target_os = "macos")]
use trigix_desktop_automation::MacosAutomationAdapter as PlatformAutomationAdapter;
#[cfg(windows)]
use trigix_desktop_automation::WindowsAutomationAdapter as PlatformAutomationAdapter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    #[cfg(not(any(windows, target_os = "macos")))]
    let adapter = PlatformAutomationAdapter::default();
    #[cfg(any(windows, target_os = "macos"))]
    let adapter = PlatformAutomationAdapter::from_environment()?;
    run_host(BufReader::new(stdin.lock()), stdout.lock(), adapter, || {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(u64::MAX)
    })?;
    Ok(())
}
