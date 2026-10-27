use desktop_release::{
    verify_release_readiness, ReadinessRejection, ReleaseManifest, ReleaseReadinessRecord,
    ReleaseTarget, RELEASE_MANIFEST_SCHEMA_VERSION,
};
use serde::de::DeserializeOwned;
use serde_json::json;
use std::env;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_INPUT_BYTES: u64 = 1024 * 1024;

struct Arguments {
    manifest: PathBuf,
    record: PathBuf,
    expected_source_revision: String,
    expected_target: ReleaseTarget,
    expected_version: String,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("desktop release readiness verification failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), &'static str> {
    let arguments = parse_arguments()?;
    let now_unix_seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "invalid_system_time")?
        .as_secs();
    let manifest: ReleaseManifest = read_bounded_json(&arguments.manifest, "invalid_manifest")?;
    let record: ReleaseReadinessRecord = read_bounded_json(&arguments.record, "invalid_record")?;

    if manifest.schema_version != RELEASE_MANIFEST_SCHEMA_VERSION {
        return Err("invalid_manifest");
    }

    if !record
        .source_revision
        .eq_ignore_ascii_case(&arguments.expected_source_revision)
    {
        return Err("source_revision_mismatch");
    }
    if record.target != arguments.expected_target || manifest.target != arguments.expected_target {
        return Err("target_mismatch");
    }
    if record.version != arguments.expected_version
        || manifest.version != arguments.expected_version
    {
        return Err("version_mismatch");
    }

    verify_release_readiness(&record, &manifest, now_unix_seconds)
        .map_err(readiness_rejection_code)?;

    let evidence = json!({
        "schema": "trigix.desktop-release-readiness-verification.v1",
        "readiness_verified": true,
        "release_id": record.release_id,
        "version": record.version,
        "target": record.target,
        "source_revision": record.source_revision,
        "artifact_sha256": record.artifact_sha256,
        "verified_at_unix_seconds": now_unix_seconds,
    });
    println!(
        "{}",
        serde_json::to_string(&evidence).map_err(|_| "output_serialization_failed")?
    );
    Ok(())
}

fn parse_arguments() -> Result<Arguments, &'static str> {
    let mut manifest = None;
    let mut record = None;
    let mut expected_source_revision = None;
    let mut expected_target = None;
    let mut expected_version = None;
    let mut arguments = env::args_os().skip(1);

    while let Some(argument) = arguments.next() {
        let name = argument.to_str().ok_or("invalid_argument")?;
        let value = arguments.next().ok_or("missing_argument_value")?;
        match name {
            "--manifest" => set_once(&mut manifest, PathBuf::from(value))?,
            "--record" => set_once(&mut record, PathBuf::from(value))?,
            "--expected-source-revision" => set_once(
                &mut expected_source_revision,
                value.into_string().map_err(|_| "invalid_source_revision")?,
            )?,
            "--expected-target" => set_once(
                &mut expected_target,
                parse_target(value.to_str().ok_or("invalid_target")?)?,
            )?,
            "--expected-version" => set_once(
                &mut expected_version,
                value.into_string().map_err(|_| "invalid_version")?,
            )?,
            _ => return Err("unknown_argument"),
        }
    }

    Ok(Arguments {
        manifest: manifest.ok_or("missing_manifest")?,
        record: record.ok_or("missing_record")?,
        expected_source_revision: expected_source_revision.ok_or("missing_source_revision")?,
        expected_target: expected_target.ok_or("missing_target")?,
        expected_version: expected_version.ok_or("missing_version")?,
    })
}

fn set_once<T>(slot: &mut Option<T>, value: T) -> Result<(), &'static str> {
    if slot.is_some() {
        return Err("duplicate_argument");
    }
    *slot = Some(value);
    Ok(())
}

fn parse_target(value: &str) -> Result<ReleaseTarget, &'static str> {
    match value {
        "windows_x86_64" => Ok(ReleaseTarget::WindowsX86_64),
        "mac_os_universal" => Ok(ReleaseTarget::MacOsUniversal),
        _ => Err("invalid_target"),
    }
}

fn read_bounded_json<T: DeserializeOwned>(
    path: &Path,
    invalid_code: &'static str,
) -> Result<T, &'static str> {
    let file = File::open(path).map_err(|_| "input_unavailable")?;
    if file.metadata().map_err(|_| "input_unavailable")?.len() > MAX_INPUT_BYTES {
        return Err("input_too_large");
    }
    let mut bytes = Vec::new();
    file.take(MAX_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "input_unavailable")?;
    if bytes.len() as u64 > MAX_INPUT_BYTES {
        return Err("input_too_large");
    }
    serde_json::from_slice(&bytes).map_err(|_| invalid_code)
}

fn readiness_rejection_code(rejection: ReadinessRejection) -> &'static str {
    match rejection {
        ReadinessRejection::InvalidRecord => "invalid_record",
        ReadinessRejection::ManifestMismatch => "manifest_mismatch",
        ReadinessRejection::Expired => "record_expired",
        ReadinessRejection::ArtifactMismatch => "artifact_mismatch",
        ReadinessRejection::ControlFailed => "control_failed",
        ReadinessRejection::EvidenceExpired => "evidence_expired",
        ReadinessRejection::UnresolvedSecurityFindings => "unresolved_security_findings",
    }
}
