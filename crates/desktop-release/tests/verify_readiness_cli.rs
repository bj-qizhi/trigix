use desktop_release::{
    ArtifactLocation, ClientArchitecture, ClientQualificationEvidence, EvidenceFormat,
    PenetrationTestDisposition, ReadinessOutcome, ReleaseArtifact, ReleaseChannel,
    ReleaseControlEvidence, ReleaseEvidence, ReleaseManifest, ReleaseReadinessRecord,
    ReleaseTarget, RELEASE_MANIFEST_SCHEMA_VERSION, RELEASE_READINESS_SCHEMA_VERSION,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const SOURCE_REVISION: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn digest(character: char) -> String {
    character.to_string().repeat(64)
}

fn control() -> ReleaseControlEvidence {
    let now = now();
    ReleaseControlEvidence {
        subject_artifact_sha256: digest('a'),
        evidence_sha256: digest('b'),
        completed_at_unix_seconds: now - 600,
        valid_until_unix_seconds: now + 7_200,
        outcome: ReadinessOutcome::Passed,
    }
}

fn qualification(os_version: &str) -> ClientQualificationEvidence {
    ClientQualificationEvidence {
        os_version: os_version.to_owned(),
        architecture: ClientArchitecture::X86_64,
        clean_install: control(),
        automation: control(),
        upgrade: control(),
        rollback: control(),
        uninstall: control(),
    }
}

fn manifest() -> ReleaseManifest {
    let now = now();
    ReleaseManifest {
        schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
        target: ReleaseTarget::WindowsX86_64,
        release_id: "stable-1.6.0-1".to_owned(),
        sequence: 1,
        version: "1.6.0".to_owned(),
        channel: ReleaseChannel::Stable,
        protocol_revision_min: 1,
        protocol_revision_max: 1,
        published_at_unix_seconds: now - 600,
        expires_at_unix_seconds: now + 7_200,
        rollout_percent: 100,
        security_deadline_unix_seconds: None,
        rollback_from_version: None,
        artifact: ReleaseArtifact {
            location: ArtifactLocation::Online {
                url: "https://releases.example.test/desktop.exe".to_owned(),
            },
            size_bytes: 1,
            sha256_hex: digest('a'),
        },
        sbom: ReleaseEvidence {
            format: EvidenceFormat::SpdxJson,
            size_bytes: 1,
            sha256_hex: digest('c'),
        },
        provenance: ReleaseEvidence {
            format: EvidenceFormat::SlsaProvenanceJson,
            size_bytes: 1,
            sha256_hex: digest('d'),
        },
        signature: "signature-verified-by-the-protected-manifest-boundary".to_owned(),
    }
}

fn record() -> ReleaseReadinessRecord {
    let now = now();
    ReleaseReadinessRecord {
        schema_version: RELEASE_READINESS_SCHEMA_VERSION,
        target: ReleaseTarget::WindowsX86_64,
        release_id: "stable-1.6.0-1".to_owned(),
        version: "1.6.0".to_owned(),
        source_revision: SOURCE_REVISION.to_owned(),
        artifact_sha256: digest('a'),
        sbom_sha256: digest('c'),
        provenance_sha256: digest('d'),
        attested_at_unix_seconds: now,
        expires_at_unix_seconds: now + 3_600,
        client_qualifications: vec![
            qualification("windows-11-24h2"),
            qualification("windows-11-25h2"),
        ],
        code_signature_verification: control(),
        notarization_verification: None,
        malware_scan: control(),
        dependency_review: control(),
        penetration_test: PenetrationTestDisposition {
            control: control(),
            unresolved_critical: 0,
            unresolved_high: 0,
            unresolved_medium: 0,
            medium_risk_acceptance_sha256: None,
        },
        release_approval: control(),
    }
}

fn test_directory(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "trigix-readiness-cli-{name}-{}",
        std::process::id()
    ));
    if path.exists() {
        fs::remove_dir_all(&path).unwrap();
    }
    fs::create_dir(&path).unwrap();
    path
}

fn write_json(path: &Path, value: &impl serde::Serialize) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}

fn invoke(directory: &Path, expected_source_revision: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_verify-readiness"))
        .args([
            "--manifest",
            directory.join("manifest.json").to_str().unwrap(),
            "--record",
            directory.join("record.json").to_str().unwrap(),
            "--expected-source-revision",
            expected_source_revision,
            "--expected-target",
            "windows_x86_64",
            "--expected-version",
            "1.6.0",
        ])
        .output()
        .unwrap()
}

#[test]
fn verifies_an_exact_release_candidate_without_echoing_private_evidence() {
    let directory = test_directory("success");
    write_json(&directory.join("manifest.json"), &manifest());
    write_json(&directory.join("record.json"), &record());

    let output = invoke(&directory, SOURCE_REVISION);
    assert!(output.status.success());
    let evidence: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(evidence["readiness_verified"], true);
    assert_eq!(evidence["source_revision"], SOURCE_REVISION);
    assert_eq!(evidence["target"], "windows_x86_64");
    assert!(evidence.get("penetration_test").is_none());
    assert!(output.stderr.is_empty());

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn rejects_source_revision_and_control_failures_with_fixed_categories() {
    let directory = test_directory("rejection");
    write_json(&directory.join("manifest.json"), &manifest());
    write_json(&directory.join("record.json"), &record());

    let mismatch = invoke(&directory, &"f".repeat(40));
    assert!(!mismatch.status.success());
    assert_eq!(
        String::from_utf8(mismatch.stderr).unwrap().trim(),
        "desktop release readiness verification failed: source_revision_mismatch"
    );

    let mut failed = record();
    failed.malware_scan.outcome = ReadinessOutcome::Failed;
    write_json(&directory.join("record.json"), &failed);
    let failed_control = invoke(&directory, SOURCE_REVISION);
    assert!(!failed_control.status.success());
    assert_eq!(
        String::from_utf8(failed_control.stderr).unwrap().trim(),
        "desktop release readiness verification failed: control_failed"
    );

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn rejects_oversized_input_before_json_parsing() {
    let directory = test_directory("oversized");
    fs::write(directory.join("manifest.json"), vec![b' '; 1024 * 1024 + 1]).unwrap();
    write_json(&directory.join("record.json"), &record());

    let output = invoke(&directory, SOURCE_REVISION);
    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap().trim(),
        "desktop release readiness verification failed: input_too_large"
    );

    fs::remove_dir_all(directory).unwrap();
}
