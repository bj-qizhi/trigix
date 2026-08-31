use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use url::Url;

pub const RELEASE_MANIFEST_SCHEMA_VERSION: u16 = 1;
pub const MAX_RELEASE_ARTIFACT_BYTES: u64 = 1024 * 1024 * 1024;
pub const MAX_EVIDENCE_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_MANIFEST_LIFETIME_SECONDS: u64 = 31 * 24 * 60 * 60;
pub const MAX_HEALTH_AGE_SECONDS: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseChannel {
    Internal,
    ClosedBeta,
    Stable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ArtifactLocation {
    Online { url: String },
    OfflineBundle { bundle_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseArtifact {
    pub location: ArtifactLocation,
    pub size_bytes: u64,
    pub sha256_hex: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceFormat {
    CycloneDxJson,
    SpdxJson,
    SlsaProvenanceJson,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseEvidence {
    pub format: EvidenceFormat,
    pub size_bytes: u64,
    pub sha256_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseManifest {
    pub schema_version: u16,
    pub release_id: String,
    pub sequence: u64,
    pub version: String,
    pub channel: ReleaseChannel,
    pub protocol_revision_min: u16,
    pub protocol_revision_max: u16,
    pub published_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub rollout_percent: u8,
    pub security_deadline_unix_seconds: Option<u64>,
    pub rollback_from_version: Option<String>,
    pub artifact: ReleaseArtifact,
    pub sbom: ReleaseEvidence,
    pub provenance: ReleaseEvidence,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
struct UnsignedReleaseManifest<'a> {
    schema_version: u16,
    release_id: &'a str,
    sequence: u64,
    version: &'a str,
    channel: ReleaseChannel,
    protocol_revision_min: u16,
    protocol_revision_max: u16,
    published_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
    rollout_percent: u8,
    security_deadline_unix_seconds: Option<u64>,
    rollback_from_version: Option<&'a str>,
    artifact: &'a ReleaseArtifact,
    sbom: &'a ReleaseEvidence,
    provenance: &'a ReleaseEvidence,
}

pub trait ManifestSignatureVerifier {
    fn verify(&self, payload: &[u8], signature: &str) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateMode {
    Disabled,
    Manual,
    Automatic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaintenanceWindow {
    pub start_minute_utc: u16,
    pub duration_minutes: u16,
}

impl MaintenanceWindow {
    pub fn contains(self, unix_seconds: u64) -> bool {
        let minute = ((unix_seconds / 60) % (24 * 60)) as u16;
        let duration = self.duration_minutes.min(24 * 60);
        (0..duration).any(|offset| (self.start_minute_utc + offset) % (24 * 60) == minute)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnterpriseUpdatePolicy {
    pub mode: UpdateMode,
    pub channel: ReleaseChannel,
    pub pinned_version: Option<String>,
    pub maintenance_window: Option<MaintenanceWindow>,
    pub allow_offline_import: bool,
    pub allow_emergency_rollback: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateContext {
    pub installed_version: String,
    pub protocol_revision: u16,
    pub device_id: String,
    pub now_unix_seconds: u64,
    pub last_accepted_sequence: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionReason {
    InvalidManifest,
    InvalidSignature,
    UntrustedOrigin,
    Expired,
    Replay,
    IncompatibleProtocol,
    Downgrade,
    RollbackNotAuthorized,
    ChannelMismatch,
    VersionPinMismatch,
    OfflineImportDisabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferralReason {
    UpdatesDisabled,
    OutsideMaintenanceWindow,
    OutsideRollout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateDecision {
    Current,
    AvailableForManualInstall,
    InstallAuthorized,
    Deferred(DeferralReason),
    Rejected(RejectionReason),
}

pub struct ReleasePolicy {
    allowed_origins: HashSet<String>,
}

impl ReleasePolicy {
    pub fn new(origins: impl IntoIterator<Item = String>) -> Result<Self, RejectionReason> {
        let mut allowed_origins = HashSet::new();
        for origin in origins {
            let url = Url::parse(&origin).map_err(|_| RejectionReason::UntrustedOrigin)?;
            if url.scheme() != "https"
                || url.path() != "/"
                || url.query().is_some()
                || url.fragment().is_some()
                || !url.username().is_empty()
                || url.password().is_some()
            {
                return Err(RejectionReason::UntrustedOrigin);
            }
            allowed_origins.insert(url.origin().ascii_serialization());
        }
        Ok(Self { allowed_origins })
    }

    pub fn decide<V: ManifestSignatureVerifier>(
        &self,
        manifest: &ReleaseManifest,
        policy: &EnterpriseUpdatePolicy,
        context: &UpdateContext,
        verifier: &V,
    ) -> UpdateDecision {
        if let Err(reason) = self.validate(manifest, policy, context, verifier) {
            return UpdateDecision::Rejected(reason);
        }
        let installed =
            Version::parse(&context.installed_version).expect("validated context version");
        let offered = Version::parse(&manifest.version).expect("validated manifest version");
        if offered == installed {
            return UpdateDecision::Current;
        }
        if policy.mode == UpdateMode::Disabled {
            return UpdateDecision::Deferred(DeferralReason::UpdatesDisabled);
        }
        if rollout_bucket(&context.device_id, &manifest.release_id) >= manifest.rollout_percent {
            return UpdateDecision::Deferred(DeferralReason::OutsideRollout);
        }
        if policy.mode == UpdateMode::Manual {
            return UpdateDecision::AvailableForManualInstall;
        }
        let deadline_reached = manifest
            .security_deadline_unix_seconds
            .is_some_and(|deadline| context.now_unix_seconds >= deadline);
        if !deadline_reached
            && policy
                .maintenance_window
                .is_some_and(|window| !window.contains(context.now_unix_seconds))
        {
            return UpdateDecision::Deferred(DeferralReason::OutsideMaintenanceWindow);
        }
        UpdateDecision::InstallAuthorized
    }

    fn validate<V: ManifestSignatureVerifier>(
        &self,
        manifest: &ReleaseManifest,
        policy: &EnterpriseUpdatePolicy,
        context: &UpdateContext,
        verifier: &V,
    ) -> Result<(), RejectionReason> {
        let offered =
            Version::parse(&manifest.version).map_err(|_| RejectionReason::InvalidManifest)?;
        let installed = Version::parse(&context.installed_version)
            .map_err(|_| RejectionReason::InvalidManifest)?;
        let pinned = policy
            .pinned_version
            .as_deref()
            .map(Version::parse)
            .transpose()
            .map_err(|_| RejectionReason::InvalidManifest)?;
        if manifest.schema_version != RELEASE_MANIFEST_SCHEMA_VERSION
            || !valid_identifier(&manifest.release_id, 96)
            || manifest.sequence == 0
            || context.device_id.is_empty()
            || context.device_id.len() > 128
            || manifest.protocol_revision_min == 0
            || manifest.protocol_revision_min > manifest.protocol_revision_max
            || manifest.published_at_unix_seconds == 0
            || manifest.expires_at_unix_seconds <= manifest.published_at_unix_seconds
            || manifest.expires_at_unix_seconds - manifest.published_at_unix_seconds
                > MAX_MANIFEST_LIFETIME_SECONDS
            || manifest.rollout_percent == 0
            || manifest.rollout_percent > 100
            || manifest.signature.is_empty()
            || manifest.signature.len() > 1_024
            || !valid_artifact(&manifest.artifact)
            || !valid_evidence(
                &manifest.sbom,
                EvidenceFormat::CycloneDxJson,
                EvidenceFormat::SpdxJson,
            )
            || manifest.provenance.format != EvidenceFormat::SlsaProvenanceJson
            || !valid_evidence(
                &manifest.provenance,
                EvidenceFormat::SlsaProvenanceJson,
                EvidenceFormat::SlsaProvenanceJson,
            )
            || policy.maintenance_window.is_some_and(|window| {
                window.start_minute_utc >= 24 * 60
                    || window.duration_minutes == 0
                    || window.duration_minutes > 12 * 60
            })
        {
            return Err(RejectionReason::InvalidManifest);
        }
        match &manifest.artifact.location {
            ArtifactLocation::Online { url } => {
                let url = Url::parse(url).map_err(|_| RejectionReason::UntrustedOrigin)?;
                if url.scheme() != "https"
                    || !url.username().is_empty()
                    || url.password().is_some()
                    || !self
                        .allowed_origins
                        .contains(&url.origin().ascii_serialization())
                {
                    return Err(RejectionReason::UntrustedOrigin);
                }
            }
            ArtifactLocation::OfflineBundle { bundle_id } => {
                if !policy.allow_offline_import || !valid_identifier(bundle_id, 128) {
                    return Err(RejectionReason::OfflineImportDisabled);
                }
            }
        }
        if context.now_unix_seconds < manifest.published_at_unix_seconds
            || context.now_unix_seconds >= manifest.expires_at_unix_seconds
        {
            return Err(RejectionReason::Expired);
        }
        if manifest.sequence <= context.last_accepted_sequence {
            return Err(RejectionReason::Replay);
        }
        if !(manifest.protocol_revision_min..=manifest.protocol_revision_max)
            .contains(&context.protocol_revision)
        {
            return Err(RejectionReason::IncompatibleProtocol);
        }
        if manifest.channel != policy.channel {
            return Err(RejectionReason::ChannelMismatch);
        }
        if pinned.as_ref().is_some_and(|version| version != &offered) {
            return Err(RejectionReason::VersionPinMismatch);
        }
        if offered < installed {
            let rollback_matches = manifest
                .rollback_from_version
                .as_deref()
                .and_then(|value| Version::parse(value).ok())
                .is_some_and(|version| version == installed);
            if !rollback_matches {
                return Err(RejectionReason::Downgrade);
            }
            if !policy.allow_emergency_rollback {
                return Err(RejectionReason::RollbackNotAuthorized);
            }
        } else if manifest.rollback_from_version.is_some() {
            return Err(RejectionReason::InvalidManifest);
        }
        let payload = signing_payload(manifest).map_err(|_| RejectionReason::InvalidManifest)?;
        if !verifier.verify(&payload, &manifest.signature) {
            return Err(RejectionReason::InvalidSignature);
        }
        Ok(())
    }
}

pub fn signing_payload(manifest: &ReleaseManifest) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(&UnsignedReleaseManifest {
        schema_version: manifest.schema_version,
        release_id: &manifest.release_id,
        sequence: manifest.sequence,
        version: &manifest.version,
        channel: manifest.channel,
        protocol_revision_min: manifest.protocol_revision_min,
        protocol_revision_max: manifest.protocol_revision_max,
        published_at_unix_seconds: manifest.published_at_unix_seconds,
        expires_at_unix_seconds: manifest.expires_at_unix_seconds,
        rollout_percent: manifest.rollout_percent,
        security_deadline_unix_seconds: manifest.security_deadline_unix_seconds,
        rollback_from_version: manifest.rollback_from_version.as_deref(),
        artifact: &manifest.artifact,
        sbom: &manifest.sbom,
        provenance: &manifest.provenance,
    })
}

pub fn validate_artifact_bytes(
    artifact: &ReleaseArtifact,
    bytes: &[u8],
) -> Result<(), RejectionReason> {
    if !valid_artifact(artifact) || bytes.len() as u64 != artifact.size_bytes {
        return Err(RejectionReason::InvalidManifest);
    }
    let actual = hex_digest(bytes);
    if !actual.eq_ignore_ascii_case(&artifact.sha256_hex) {
        return Err(RejectionReason::InvalidManifest);
    }
    Ok(())
}

fn valid_artifact(artifact: &ReleaseArtifact) -> bool {
    artifact.size_bytes > 0
        && artifact.size_bytes <= MAX_RELEASE_ARTIFACT_BYTES
        && valid_digest(&artifact.sha256_hex)
}

fn valid_evidence(
    evidence: &ReleaseEvidence,
    first: EvidenceFormat,
    second: EvidenceFormat,
) -> bool {
    matches!(evidence.format, value if value == first || value == second)
        && evidence.size_bytes > 0
        && evidence.size_bytes <= MAX_EVIDENCE_BYTES
        && valid_digest(&evidence.sha256_hex)
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_identifier(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn hex_digest(value: &[u8]) -> String {
    Sha256::digest(value)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn rollout_bucket(device_id: &str, release_id: &str) -> u8 {
    let digest = Sha256::digest(format!("{device_id}\0{release_id}").as_bytes());
    (u16::from_be_bytes([digest[0], digest[1]]) % 100) as u8
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FleetLifecycle {
    Active,
    Suspended,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetDeviceVersion {
    pub device_id: String,
    pub agent_version: String,
    pub last_seen_at_unix_seconds: u64,
    pub lifecycle: FleetLifecycle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FleetCompliance {
    Compliant,
    UpdateRequired,
    AheadOfPolicy,
    Stale,
    Suspended,
    Revoked,
    InvalidInventory,
}

pub fn classify_fleet_device(
    device: &FleetDeviceVersion,
    required_version: &str,
    now_unix_seconds: u64,
) -> FleetCompliance {
    if device.device_id.is_empty() || device.device_id.len() > 128 {
        return FleetCompliance::InvalidInventory;
    }
    match device.lifecycle {
        FleetLifecycle::Suspended => return FleetCompliance::Suspended,
        FleetLifecycle::Revoked => return FleetCompliance::Revoked,
        FleetLifecycle::Active => {}
    }
    if device.last_seen_at_unix_seconds > now_unix_seconds
        || now_unix_seconds - device.last_seen_at_unix_seconds > MAX_HEALTH_AGE_SECONDS
    {
        return FleetCompliance::Stale;
    }
    let Ok(installed) = Version::parse(&device.agent_version) else {
        return FleetCompliance::InvalidInventory;
    };
    let Ok(required) = Version::parse(required_version) else {
        return FleetCompliance::InvalidInventory;
    };
    match installed.cmp(&required) {
        std::cmp::Ordering::Less => FleetCompliance::UpdateRequired,
        std::cmp::Ordering::Equal => FleetCompliance::Compliant,
        std::cmp::Ordering::Greater => FleetCompliance::AheadOfPolicy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestVerifier;
    impl ManifestSignatureVerifier for TestVerifier {
        fn verify(&self, payload: &[u8], signature: &str) -> bool {
            signature == hex_digest(payload)
        }
    }

    fn manifest() -> ReleaseManifest {
        let bytes = b"signed-installer-fixture";
        let mut value = ReleaseManifest {
            schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
            release_id: "stable-1.6.0-1".to_owned(),
            sequence: 12,
            version: "1.6.0".to_owned(),
            channel: ReleaseChannel::Stable,
            protocol_revision_min: 1,
            protocol_revision_max: 2,
            published_at_unix_seconds: 1_000,
            expires_at_unix_seconds: 1_000 + 7 * 24 * 60 * 60,
            rollout_percent: 100,
            security_deadline_unix_seconds: None,
            rollback_from_version: None,
            artifact: ReleaseArtifact {
                location: ArtifactLocation::Online {
                    url: "https://releases.example.test/desktop-1.6.0.exe".to_owned(),
                },
                size_bytes: bytes.len() as u64,
                sha256_hex: hex_digest(bytes),
            },
            sbom: ReleaseEvidence {
                format: EvidenceFormat::CycloneDxJson,
                size_bytes: 1_024,
                sha256_hex: "a".repeat(64),
            },
            provenance: ReleaseEvidence {
                format: EvidenceFormat::SlsaProvenanceJson,
                size_bytes: 2_048,
                sha256_hex: "b".repeat(64),
            },
            signature: String::new(),
        };
        value.signature = hex_digest(&signing_payload(&value).unwrap());
        value
    }

    fn policy() -> EnterpriseUpdatePolicy {
        EnterpriseUpdatePolicy {
            mode: UpdateMode::Automatic,
            channel: ReleaseChannel::Stable,
            pinned_version: None,
            maintenance_window: None,
            allow_offline_import: false,
            allow_emergency_rollback: false,
        }
    }

    fn context() -> UpdateContext {
        UpdateContext {
            installed_version: "1.5.1".to_owned(),
            protocol_revision: 2,
            device_id: "device-001".to_owned(),
            now_unix_seconds: 2_000,
            last_accepted_sequence: 11,
        }
    }

    fn release_policy() -> ReleasePolicy {
        ReleasePolicy::new(["https://releases.example.test".to_owned()]).unwrap()
    }

    #[test]
    fn signed_compatible_release_authorizes_without_installing() {
        assert_eq!(
            release_policy().decide(&manifest(), &policy(), &context(), &TestVerifier),
            UpdateDecision::InstallAuthorized
        );
        validate_artifact_bytes(&manifest().artifact, b"signed-installer-fixture").unwrap();
    }

    #[test]
    fn tamper_replay_expiry_protocol_and_origin_fail_closed() {
        let cases = [
            (
                ReleaseManifest {
                    version: "1.6.1".to_owned(),
                    ..manifest()
                },
                RejectionReason::InvalidSignature,
            ),
            (manifest(), RejectionReason::Replay),
            (manifest(), RejectionReason::Expired),
            (manifest(), RejectionReason::IncompatibleProtocol),
            (
                ReleaseManifest {
                    artifact: ReleaseArtifact {
                        location: ArtifactLocation::Online {
                            url: "https://public.example.test/release.exe".to_owned(),
                        },
                        ..manifest().artifact
                    },
                    ..manifest()
                },
                RejectionReason::UntrustedOrigin,
            ),
        ];
        let mut contexts = [context(), context(), context(), context(), context()];
        contexts[1].last_accepted_sequence = 12;
        contexts[2].now_unix_seconds = manifest().expires_at_unix_seconds;
        contexts[3].protocol_revision = 3;
        for ((candidate, reason), candidate_context) in cases.into_iter().zip(contexts) {
            assert_eq!(
                release_policy().decide(&candidate, &policy(), &candidate_context, &TestVerifier),
                UpdateDecision::Rejected(reason)
            );
        }
    }

    #[test]
    fn centralized_modes_pin_windows_deadline_and_offline_import_are_deterministic() {
        assert_eq!(
            release_policy().decide(
                &manifest(),
                &EnterpriseUpdatePolicy {
                    mode: UpdateMode::Disabled,
                    ..policy()
                },
                &context(),
                &TestVerifier
            ),
            UpdateDecision::Deferred(DeferralReason::UpdatesDisabled)
        );
        assert_eq!(
            release_policy().decide(
                &manifest(),
                &EnterpriseUpdatePolicy {
                    mode: UpdateMode::Manual,
                    ..policy()
                },
                &context(),
                &TestVerifier
            ),
            UpdateDecision::AvailableForManualInstall
        );
        let outside = EnterpriseUpdatePolicy {
            maintenance_window: Some(MaintenanceWindow {
                start_minute_utc: 60,
                duration_minutes: 30,
            }),
            ..policy()
        };
        assert_eq!(
            release_policy().decide(&manifest(), &outside, &context(), &TestVerifier),
            UpdateDecision::Deferred(DeferralReason::OutsideMaintenanceWindow)
        );
        let deadline = ReleaseManifest {
            security_deadline_unix_seconds: Some(1_500),
            ..manifest()
        };
        let deadline = ReleaseManifest {
            signature: hex_digest(&signing_payload(&deadline).unwrap()),
            ..deadline
        };
        assert_eq!(
            release_policy().decide(&deadline, &outside, &context(), &TestVerifier),
            UpdateDecision::InstallAuthorized
        );
        assert_eq!(
            release_policy().decide(
                &manifest(),
                &EnterpriseUpdatePolicy {
                    pinned_version: Some("1.5.9".to_owned()),
                    ..policy()
                },
                &context(),
                &TestVerifier
            ),
            UpdateDecision::Rejected(RejectionReason::VersionPinMismatch)
        );
        let offline = ReleaseManifest {
            artifact: ReleaseArtifact {
                location: ArtifactLocation::OfflineBundle {
                    bundle_id: "airgap-1.6.0".to_owned(),
                },
                ..manifest().artifact
            },
            ..manifest()
        };
        let offline = ReleaseManifest {
            signature: hex_digest(&signing_payload(&offline).unwrap()),
            ..offline
        };
        assert_eq!(
            release_policy().decide(&offline, &policy(), &context(), &TestVerifier),
            UpdateDecision::Rejected(RejectionReason::OfflineImportDisabled)
        );
        assert_eq!(
            release_policy().decide(
                &offline,
                &EnterpriseUpdatePolicy {
                    allow_offline_import: true,
                    ..policy()
                },
                &context(),
                &TestVerifier
            ),
            UpdateDecision::InstallAuthorized
        );
    }

    #[test]
    fn rollout_and_maintenance_clock_boundaries_are_stable() {
        let staged = ReleaseManifest {
            rollout_percent: 1,
            ..manifest()
        };
        let staged = ReleaseManifest {
            signature: hex_digest(&signing_payload(&staged).unwrap()),
            ..staged
        };
        let device_id = (0..1_000)
            .map(|value| format!("device-{value}"))
            .find(|device| rollout_bucket(device, &staged.release_id) >= staged.rollout_percent)
            .unwrap();
        assert_eq!(
            release_policy().decide(
                &staged,
                &policy(),
                &UpdateContext {
                    device_id,
                    ..context()
                },
                &TestVerifier
            ),
            UpdateDecision::Deferred(DeferralReason::OutsideRollout)
        );

        let timed = EnterpriseUpdatePolicy {
            maintenance_window: Some(MaintenanceWindow {
                start_minute_utc: 33,
                duration_minutes: 1,
            }),
            ..policy()
        };
        assert_eq!(
            release_policy().decide(
                &manifest(),
                &timed,
                &UpdateContext {
                    now_unix_seconds: 33 * 60,
                    ..context()
                },
                &TestVerifier
            ),
            UpdateDecision::InstallAuthorized
        );
        assert_eq!(
            release_policy().decide(
                &manifest(),
                &timed,
                &UpdateContext {
                    now_unix_seconds: 34 * 60,
                    ..context()
                },
                &TestVerifier
            ),
            UpdateDecision::Deferred(DeferralReason::OutsideMaintenanceWindow)
        );
    }

    #[test]
    fn downgrade_requires_explicit_matching_rollback_and_central_authority() {
        let rollback = ReleaseManifest {
            version: "1.4.9".to_owned(),
            rollback_from_version: Some("1.5.1".to_owned()),
            ..manifest()
        };
        let rollback = ReleaseManifest {
            signature: hex_digest(&signing_payload(&rollback).unwrap()),
            ..rollback
        };
        assert_eq!(
            release_policy().decide(&rollback, &policy(), &context(), &TestVerifier),
            UpdateDecision::Rejected(RejectionReason::RollbackNotAuthorized)
        );
        assert_eq!(
            release_policy().decide(
                &rollback,
                &EnterpriseUpdatePolicy {
                    allow_emergency_rollback: true,
                    ..policy()
                },
                &context(),
                &TestVerifier
            ),
            UpdateDecision::InstallAuthorized
        );
    }

    #[test]
    fn fleet_compliance_is_bounded_and_lifecycle_aware() {
        let device = FleetDeviceVersion {
            device_id: "device-001".to_owned(),
            agent_version: "1.5.1".to_owned(),
            last_seen_at_unix_seconds: 10_000,
            lifecycle: FleetLifecycle::Active,
        };
        assert_eq!(
            classify_fleet_device(&device, "1.5.1", 10_001),
            FleetCompliance::Compliant
        );
        assert_eq!(
            classify_fleet_device(&device, "1.6.0", 10_001),
            FleetCompliance::UpdateRequired
        );
        assert_eq!(
            classify_fleet_device(
                &FleetDeviceVersion {
                    lifecycle: FleetLifecycle::Revoked,
                    ..device.clone()
                },
                "1.5.1",
                10_001
            ),
            FleetCompliance::Revoked
        );
        assert_eq!(
            classify_fleet_device(&device, "1.5.1", 10_000 + MAX_HEALTH_AGE_SECONDS + 1),
            FleetCompliance::Stale
        );
    }

    #[test]
    fn manifest_contract_rejects_unknown_authority_fields_and_long_windows() {
        let mut value = serde_json::to_value(manifest()).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("installer_command".to_owned(), serde_json::json!("run"));
        assert!(serde_json::from_value::<ReleaseManifest>(value).is_err());
        let too_long = ReleaseManifest {
            expires_at_unix_seconds: 1_000 + MAX_MANIFEST_LIFETIME_SECONDS + 1,
            ..manifest()
        };
        assert_eq!(
            release_policy().decide(&too_long, &policy(), &context(), &TestVerifier),
            UpdateDecision::Rejected(RejectionReason::InvalidManifest)
        );
    }
}
