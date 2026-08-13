use std::io::{self, BufReader};
use std::time::{SystemTime, UNIX_EPOCH};
use trigix_desktop_automation::{run_host, FixtureAutomationAdapter};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_host(
        BufReader::new(stdin.lock()),
        stdout.lock(),
        FixtureAutomationAdapter,
        || {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_millis() as u64)
                .unwrap_or(u64::MAX)
        },
    )?;
    Ok(())
}
