use std::io::{self, BufReader};
use std::time::{SystemTime, UNIX_EPOCH};
use trigix_desktop_automation::run_host;

#[cfg(not(windows))]
use trigix_desktop_automation::FixtureAutomationAdapter as PlatformAutomationAdapter;
#[cfg(windows)]
use trigix_desktop_automation::WindowsAutomationAdapter as PlatformAutomationAdapter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_host(
        BufReader::new(stdin.lock()),
        stdout.lock(),
        PlatformAutomationAdapter::default(),
        || {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_millis() as u64)
                .unwrap_or(u64::MAX)
        },
    )?;
    Ok(())
}
