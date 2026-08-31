use desktop_protocol::{
    AvatarEmotion, AvatarFailureCategory, AvatarPresentationEvent, AvatarPresentationEventKind,
    AvatarPresentationState, AvatarViseme, ProtocolError,
};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use url::Url;

pub const MAX_PACKAGE_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_STARTUP_MS: u32 = 2_000;
pub const MAX_FRAME_TIME_P95_MICROS: u32 = 33_334;
pub const MAX_MEMORY_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_DROPPED_FRAME_PERCENT: u8 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionPreference {
    Full,
    Reduced,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AvatarPreferences {
    pub enabled: bool,
    pub voice_playback: bool,
    pub motion: MotionPreference,
    pub captions: bool,
    pub high_contrast: bool,
}

impl Default for AvatarPreferences {
    fn default() -> Self {
        Self {
            enabled: true,
            voice_playback: true,
            motion: MotionPreference::Full,
            captions: true,
            high_contrast: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererMetrics {
    pub startup_ms: u32,
    pub frame_time_p95_micros: u32,
    pub memory_bytes: u64,
    pub dropped_frame_percent: u8,
    pub resize_recovered: bool,
    pub device_loss_recovered: bool,
    pub background_suspended: bool,
    pub interruption_recovered: bool,
    pub long_session_minutes: u16,
}

impl RendererMetrics {
    pub fn qualifies(self) -> bool {
        self.startup_ms <= MAX_STARTUP_MS
            && self.frame_time_p95_micros <= MAX_FRAME_TIME_P95_MICROS
            && self.memory_bytes <= MAX_MEMORY_BYTES
            && self.dropped_frame_percent <= MAX_DROPPED_FRAME_PERCENT
            && self.resize_recovered
            && self.device_loss_recovered
            && self.background_suspended
            && self.interruption_recovered
            && self.long_session_minutes >= 60
    }
}

pub fn qualify_builtin_renderer(metrics: RendererMetrics) -> bool {
    if !metrics.qualifies() {
        return false;
    }
    let make_event = |sequence, event| AvatarPresentationEvent {
        schema_version: desktop_protocol::AVATAR_EVENT_SCHEMA_VERSION,
        session_id: "qualification-session".to_owned(),
        sequence,
        occurred_at_unix_ms: 10_000 + u64::from(sequence),
        event,
    };
    let mut renderer = AvatarRenderer::new(AvatarPreferences::default());
    let result = renderer
        .apply(make_event(
            1,
            AvatarPresentationEventKind::StateChanged {
                state: AvatarPresentationState::Speaking,
            },
        ))
        .and_then(|_| {
            renderer.apply(make_event(
                2,
                AvatarPresentationEventKind::Viseme {
                    viseme: AvatarViseme::Open,
                    duration_ms: 80,
                    intensity: 70,
                },
            ))
        })
        .and_then(|_| renderer.apply(make_event(3, AvatarPresentationEventKind::Interrupted)));
    if result.is_err() || renderer.frame.viseme != AvatarViseme::Rest {
        return false;
    }
    renderer.suspend_background();
    renderer.recover_device_loss();
    renderer.frame.state == AvatarPresentationState::Idle
        && renderer.frame.failed.is_none()
        && renderer.frame.viseme == AvatarViseme::Rest
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AvatarError {
    Protocol(ProtocolError),
    InvalidState,
    RendererNotQualified,
    PackageRejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AvatarFrame {
    pub state: AvatarPresentationState,
    pub emotion: AvatarEmotion,
    pub emotion_intensity: u8,
    pub viseme: AvatarViseme,
    pub viseme_intensity: u8,
    pub failed: Option<AvatarFailureCategory>,
}

pub struct AvatarRenderer {
    preferences: AvatarPreferences,
    frame: AvatarFrame,
    previous: Option<AvatarPresentationEvent>,
}

impl AvatarRenderer {
    pub fn new(preferences: AvatarPreferences) -> Self {
        Self {
            preferences,
            frame: AvatarFrame {
                state: AvatarPresentationState::Idle,
                emotion: AvatarEmotion::Neutral,
                emotion_intensity: 0,
                viseme: AvatarViseme::Rest,
                viseme_intensity: 0,
                failed: None,
            },
            previous: None,
        }
    }

    pub fn frame(&self) -> AvatarFrame {
        self.frame
    }

    pub fn preferences(&self) -> AvatarPreferences {
        self.preferences
    }

    pub fn update_preferences(&mut self, preferences: AvatarPreferences) {
        self.preferences = preferences;
        if !preferences.enabled {
            self.stop();
        }
    }

    pub fn apply(&mut self, event: AvatarPresentationEvent) -> Result<AvatarFrame, AvatarError> {
        if !self.preferences.enabled || self.frame.state == AvatarPresentationState::Stopped {
            return Err(AvatarError::InvalidState);
        }
        event.validate().map_err(AvatarError::Protocol)?;
        if let Some(previous) = &self.previous {
            event
                .validate_after(previous)
                .map_err(AvatarError::Protocol)?;
        }
        match event.event {
            AvatarPresentationEventKind::StateChanged { state } => {
                self.frame.state = state;
                if state != AvatarPresentationState::Speaking {
                    self.frame.viseme = AvatarViseme::Rest;
                    self.frame.viseme_intensity = 0;
                }
            }
            AvatarPresentationEventKind::Viseme {
                viseme, intensity, ..
            } => {
                if self.frame.state != AvatarPresentationState::Speaking {
                    return Err(AvatarError::InvalidState);
                }
                self.frame.viseme = if self.preferences.motion == MotionPreference::None {
                    AvatarViseme::Rest
                } else {
                    viseme
                };
                self.frame.viseme_intensity = intensity;
            }
            AvatarPresentationEventKind::EmotionChanged { emotion, intensity } => {
                self.frame.emotion = emotion;
                self.frame.emotion_intensity = intensity;
            }
            AvatarPresentationEventKind::Interrupted => {
                self.frame.state = AvatarPresentationState::Interrupted;
                self.frame.viseme = AvatarViseme::Rest;
                self.frame.viseme_intensity = 0;
            }
            AvatarPresentationEventKind::Failed { category } => {
                self.frame.state = AvatarPresentationState::Error;
                self.frame.failed = Some(category);
            }
            AvatarPresentationEventKind::Stopped => self.stop(),
        }
        self.previous = Some(event);
        Ok(self.frame)
    }

    pub fn suspend_background(&mut self) {
        self.frame.state = AvatarPresentationState::Idle;
        self.frame.viseme = AvatarViseme::Rest;
        self.frame.viseme_intensity = 0;
    }

    pub fn recover_device_loss(&mut self) {
        self.frame = AvatarFrame {
            state: AvatarPresentationState::Idle,
            emotion: AvatarEmotion::Neutral,
            emotion_intensity: 0,
            viseme: AvatarViseme::Rest,
            viseme_intensity: 0,
            failed: None,
        };
    }

    pub fn stop(&mut self) {
        self.frame.state = AvatarPresentationState::Stopped;
        self.frame.viseme = AvatarViseme::Rest;
        self.frame.viseme_intensity = 0;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvatarPackageManifest {
    pub package_id: String,
    pub version: String,
    pub source_url: String,
    pub size_bytes: u64,
    pub sha256_hex: String,
    pub signature: String,
}

pub trait PackageSignatureVerifier {
    fn verify(&self, manifest: &AvatarPackageManifest, digest: &[u8; 32]) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageSelection {
    VerifiedPrivatePackage,
    BuiltInFallback,
}

pub struct PackagePolicy {
    allowed_origins: HashSet<String>,
}

impl PackagePolicy {
    pub fn new(origins: impl IntoIterator<Item = String>) -> Result<Self, AvatarError> {
        let mut allowed_origins = HashSet::new();
        for origin in origins {
            let url = Url::parse(&origin).map_err(|_| AvatarError::PackageRejected)?;
            if url.scheme() != "https"
                || url.path() != "/"
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err(AvatarError::PackageRejected);
            }
            allowed_origins.insert(url.origin().ascii_serialization());
        }
        if allowed_origins.is_empty() {
            return Err(AvatarError::PackageRejected);
        }
        Ok(Self { allowed_origins })
    }

    pub fn validate<V: PackageSignatureVerifier>(
        &self,
        manifest: &AvatarPackageManifest,
        bytes: &[u8],
        verifier: &V,
    ) -> Result<(), AvatarError> {
        let source = Url::parse(&manifest.source_url).map_err(|_| AvatarError::PackageRejected)?;
        let valid_id = !manifest.package_id.is_empty()
            && manifest.package_id.len() <= 96
            && manifest
                .package_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
        let valid_version = !manifest.version.is_empty()
            && manifest.version.len() <= 64
            && manifest.version.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'+')
            });
        if source.scheme() != "https"
            || !self
                .allowed_origins
                .contains(&source.origin().ascii_serialization())
            || !valid_id
            || !valid_version
            || manifest.size_bytes == 0
            || manifest.size_bytes > MAX_PACKAGE_BYTES
            || manifest.size_bytes != bytes.len() as u64
            || manifest.signature.is_empty()
            || manifest.signature.len() > 512
        {
            return Err(AvatarError::PackageRejected);
        }
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        let computed = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if manifest.sha256_hex.len() != 64
            || !manifest
                .sha256_hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || !computed.eq_ignore_ascii_case(&manifest.sha256_hex)
            || !verifier.verify(manifest, &digest)
        {
            return Err(AvatarError::PackageRejected);
        }
        Ok(())
    }

    pub fn select<V: PackageSignatureVerifier>(
        &self,
        manifest: &AvatarPackageManifest,
        bytes: &[u8],
        verifier: &V,
    ) -> PackageSelection {
        if self.validate(manifest, bytes, verifier).is_ok() {
            PackageSelection::VerifiedPrivatePackage
        } else {
            PackageSelection::BuiltInFallback
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use desktop_protocol::{AvatarPresentationEventKind, AVATAR_EVENT_SCHEMA_VERSION};

    fn event(sequence: u32, kind: AvatarPresentationEventKind) -> AvatarPresentationEvent {
        AvatarPresentationEvent {
            schema_version: AVATAR_EVENT_SCHEMA_VERSION,
            session_id: "avatar-session-1".to_owned(),
            sequence,
            occurred_at_unix_ms: 1_000 + u64::from(sequence),
            event: kind,
        }
    }

    #[test]
    fn renderer_is_presentation_only_ordered_and_interruptible() {
        let mut renderer = AvatarRenderer::new(AvatarPreferences::default());
        renderer
            .apply(event(
                1,
                AvatarPresentationEventKind::StateChanged {
                    state: AvatarPresentationState::Speaking,
                },
            ))
            .unwrap();
        renderer
            .apply(event(
                2,
                AvatarPresentationEventKind::Viseme {
                    viseme: AvatarViseme::Rounded,
                    duration_ms: 80,
                    intensity: 70,
                },
            ))
            .unwrap();
        assert_eq!(renderer.frame().viseme, AvatarViseme::Rounded);
        renderer
            .apply(event(3, AvatarPresentationEventKind::Interrupted))
            .unwrap();
        assert_eq!(renderer.frame().viseme, AvatarViseme::Rest);
        assert_eq!(
            renderer.apply(event(5, AvatarPresentationEventKind::Stopped)),
            Err(AvatarError::Protocol(ProtocolError::InvalidField(
                "avatar.order"
            )))
        );
        renderer
            .apply(event(4, AvatarPresentationEventKind::Stopped))
            .unwrap();
        assert_eq!(renderer.frame().state, AvatarPresentationState::Stopped);
    }

    #[test]
    fn preferences_disable_motion_and_stop_immediately() {
        let mut renderer = AvatarRenderer::new(AvatarPreferences {
            motion: MotionPreference::None,
            ..AvatarPreferences::default()
        });
        renderer
            .apply(event(
                1,
                AvatarPresentationEventKind::StateChanged {
                    state: AvatarPresentationState::Speaking,
                },
            ))
            .unwrap();
        renderer
            .apply(event(
                2,
                AvatarPresentationEventKind::Viseme {
                    viseme: AvatarViseme::Open,
                    duration_ms: 60,
                    intensity: 80,
                },
            ))
            .unwrap();
        assert_eq!(renderer.frame().viseme, AvatarViseme::Rest);
        renderer.update_preferences(AvatarPreferences {
            enabled: false,
            ..renderer.preferences()
        });
        assert_eq!(renderer.frame().state, AvatarPresentationState::Stopped);
    }

    #[test]
    fn qualification_enforces_every_performance_and_recovery_gate() {
        let qualified = RendererMetrics {
            startup_ms: 100,
            frame_time_p95_micros: 16_667,
            memory_bytes: 32 * 1024 * 1024,
            dropped_frame_percent: 1,
            resize_recovered: true,
            device_loss_recovered: true,
            background_suspended: true,
            interruption_recovered: true,
            long_session_minutes: 60,
        };
        assert!(qualified.qualifies());
        assert!(qualify_builtin_renderer(qualified));
        assert!(!RendererMetrics {
            memory_bytes: MAX_MEMORY_BYTES + 1,
            ..qualified
        }
        .qualifies());
        assert!(!RendererMetrics {
            device_loss_recovered: false,
            ..qualified
        }
        .qualifies());
    }

    struct TestVerifier(bool);
    impl PackageSignatureVerifier for TestVerifier {
        fn verify(&self, manifest: &AvatarPackageManifest, _digest: &[u8; 32]) -> bool {
            self.0 && manifest.signature == "test-signature"
        }
    }

    #[test]
    fn private_package_requires_origin_digest_size_and_signature() {
        let bytes = b"licensed-private-avatar-package";
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        let manifest = AvatarPackageManifest {
            package_id: "tenant.avatar-1".to_owned(),
            version: "1.0.0".to_owned(),
            source_url: "https://assets.example.test/avatar.bin".to_owned(),
            size_bytes: bytes.len() as u64,
            sha256_hex: digest.iter().map(|byte| format!("{byte:02x}")).collect(),
            signature: "test-signature".to_owned(),
        };
        let policy = PackagePolicy::new(["https://assets.example.test".to_owned()]).unwrap();
        assert_eq!(
            policy.validate(&manifest, bytes, &TestVerifier(true)),
            Ok(())
        );
        assert_eq!(
            policy.validate(
                &AvatarPackageManifest {
                    source_url: "https://public.example.test/avatar.bin".to_owned(),
                    ..manifest.clone()
                },
                bytes,
                &TestVerifier(true)
            ),
            Err(AvatarError::PackageRejected)
        );
        assert_eq!(
            policy.validate(&manifest, b"tampered", &TestVerifier(true)),
            Err(AvatarError::PackageRejected)
        );
        assert_eq!(
            policy.validate(&manifest, bytes, &TestVerifier(false)),
            Err(AvatarError::PackageRejected)
        );
        assert_eq!(
            policy.select(&manifest, bytes, &TestVerifier(false)),
            PackageSelection::BuiltInFallback
        );
    }

    #[test]
    fn long_session_keeps_only_the_latest_bounded_event() {
        let mut renderer = AvatarRenderer::new(AvatarPreferences::default());
        renderer
            .apply(event(
                1,
                AvatarPresentationEventKind::StateChanged {
                    state: AvatarPresentationState::Speaking,
                },
            ))
            .unwrap();
        for sequence in 2..=36_001 {
            renderer
                .apply(event(
                    sequence,
                    AvatarPresentationEventKind::Viseme {
                        viseme: AvatarViseme::Open,
                        duration_ms: 100,
                        intensity: 60,
                    },
                ))
                .unwrap();
        }
        assert_eq!(renderer.previous.as_ref().unwrap().sequence, 36_001);
        assert_eq!(renderer.frame().state, AvatarPresentationState::Speaking);
    }
}
