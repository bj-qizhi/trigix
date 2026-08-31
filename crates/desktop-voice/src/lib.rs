use desktop_protocol::{
    ProtocolError, VoiceConversationEvent, VoiceConversationEventKind, VoiceFailureCategory,
    VoiceInterruptionReason, VoiceSessionState, VoiceStopReason, VOICE_EVENT_SCHEMA_VERSION,
};

pub const MAX_PROVIDER_TEXT_BYTES: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceProviderError {
    pub category: VoiceFailureCategory,
    pub retryable: bool,
}

pub trait RecognitionProvider {
    fn start(&mut self, session_id: &str) -> Result<(), VoiceProviderError>;
    fn push_audio(&mut self, samples: &[i16]) -> Result<(), VoiceProviderError>;
    fn finish_utterance(&mut self) -> Result<(), VoiceProviderError>;
    fn reconnect(&mut self, session_id: &str) -> Result<(), VoiceProviderError>;
    fn stop(&mut self);
}

pub trait SynthesisProvider {
    fn synthesize(&mut self, text: &str) -> Result<(), VoiceProviderError>;
    fn cancel(&mut self);
    fn stop(&mut self);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VadConfig {
    pub speech_threshold_rms: u16,
    pub onset_frames: u8,
    pub hangover_frames: u8,
    pub max_frame_samples: usize,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            speech_threshold_rms: 900,
            onset_frames: 3,
            hangover_frames: 8,
            max_frame_samples: 4_800,
        }
    }
}

impl VadConfig {
    pub fn validate(self) -> Result<Self, VoiceRuntimeError> {
        if !(100..=16_000).contains(&self.speech_threshold_rms)
            || !(1..=20).contains(&self.onset_frames)
            || !(1..=50).contains(&self.hangover_frames)
            || !(80..=4_800).contains(&self.max_frame_samples)
        {
            return Err(VoiceRuntimeError::InvalidConfiguration);
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VadEvent {
    SpeechStarted,
    SpeechEnded,
}

pub struct VoiceActivityDetector {
    config: VadConfig,
    speaking: bool,
    loud_frames: u8,
    quiet_frames: u8,
}

impl VoiceActivityDetector {
    pub fn new(config: VadConfig) -> Result<Self, VoiceRuntimeError> {
        Ok(Self {
            config: config.validate()?,
            speaking: false,
            loud_frames: 0,
            quiet_frames: 0,
        })
    }

    pub fn push_frame(&mut self, samples: &[i16]) -> Result<Option<VadEvent>, VoiceRuntimeError> {
        self.validate_frame(samples)?;
        let energy = samples
            .iter()
            .map(|sample| i64::from(*sample) * i64::from(*sample))
            .sum::<i64>()
            / samples.len() as i64;
        let rms = integer_sqrt(energy as u64);
        let loud = rms >= u64::from(self.config.speech_threshold_rms);

        if self.speaking {
            if loud {
                self.quiet_frames = 0;
            } else {
                self.quiet_frames = self.quiet_frames.saturating_add(1);
                if self.quiet_frames >= self.config.hangover_frames {
                    self.speaking = false;
                    self.loud_frames = 0;
                    self.quiet_frames = 0;
                    return Ok(Some(VadEvent::SpeechEnded));
                }
            }
        } else if loud {
            self.loud_frames = self.loud_frames.saturating_add(1);
            if self.loud_frames >= self.config.onset_frames {
                self.speaking = true;
                self.loud_frames = 0;
                return Ok(Some(VadEvent::SpeechStarted));
            }
        } else {
            self.loud_frames = 0;
        }
        Ok(None)
    }

    fn validate_frame(&self, samples: &[i16]) -> Result<(), VoiceRuntimeError> {
        if samples.is_empty() || samples.len() > self.config.max_frame_samples {
            return Err(VoiceRuntimeError::InvalidAudioFrame);
        }
        Ok(())
    }

    pub fn reset(&mut self) {
        self.speaking = false;
        self.loud_frames = 0;
        self.quiet_frames = 0;
    }
}

fn integer_sqrt(value: u64) -> u64 {
    if value < 2 {
        return value;
    }
    let mut estimate = value;
    let mut next = (estimate + value / estimate) / 2;
    while next < estimate {
        estimate = next;
        next = (estimate + value / estimate) / 2;
    }
    estimate
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconnectPolicy {
    pub maximum_attempts: u8,
    pub initial_delay_ms: u32,
    pub maximum_delay_ms: u32,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            maximum_attempts: 5,
            initial_delay_ms: 250,
            maximum_delay_ms: 8_000,
        }
    }
}

impl ReconnectPolicy {
    pub fn validate(self) -> Result<Self, VoiceRuntimeError> {
        if !(1..=8).contains(&self.maximum_attempts)
            || !(100..=5_000).contains(&self.initial_delay_ms)
            || self.maximum_delay_ms < self.initial_delay_ms
            || self.maximum_delay_ms > 30_000
        {
            return Err(VoiceRuntimeError::InvalidConfiguration);
        }
        Ok(self)
    }

    pub fn delay_ms(self, attempt: u8) -> Option<u32> {
        if attempt == 0 || attempt > self.maximum_attempts {
            return None;
        }
        let multiplier = 1_u32
            .checked_shl(u32::from(attempt - 1))
            .unwrap_or(u32::MAX);
        Some(
            self.initial_delay_ms
                .saturating_mul(multiplier)
                .min(self.maximum_delay_ms),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceRuntimeError {
    InvalidConfiguration,
    InvalidAudioFrame,
    InvalidState,
    InvalidText,
    Provider(VoiceProviderError),
    Protocol(ProtocolError),
}

pub struct VoiceCoordinator<R, S> {
    recognition: R,
    synthesis: S,
    vad: VoiceActivityDetector,
    reconnect_policy: ReconnectPolicy,
    session_id: String,
    state: VoiceSessionState,
    sequence: u32,
    last_event_at_unix_ms: u64,
    reconnect_attempt: u8,
    stopped: bool,
}

impl<R: RecognitionProvider, S: SynthesisProvider> VoiceCoordinator<R, S> {
    pub fn new(
        recognition: R,
        synthesis: S,
        session_id: impl Into<String>,
        vad_config: VadConfig,
        reconnect_policy: ReconnectPolicy,
    ) -> Result<Self, VoiceRuntimeError> {
        let session_id = session_id.into();
        let probe = VoiceConversationEvent {
            schema_version: VOICE_EVENT_SCHEMA_VERSION,
            session_id: session_id.clone(),
            sequence: 1,
            occurred_at_unix_ms: 1,
            event: VoiceConversationEventKind::PermissionRequested,
        };
        probe.validate().map_err(VoiceRuntimeError::Protocol)?;
        Ok(Self {
            recognition,
            synthesis,
            vad: VoiceActivityDetector::new(vad_config)?,
            reconnect_policy: reconnect_policy.validate()?,
            session_id,
            state: VoiceSessionState::RequestingPermission,
            sequence: 0,
            last_event_at_unix_ms: 0,
            reconnect_attempt: 0,
            stopped: false,
        })
    }

    pub fn state(&self) -> VoiceSessionState {
        self.state
    }

    pub fn start(&mut self, now_unix_ms: u64) -> Result<VoiceConversationEvent, VoiceRuntimeError> {
        if self.stopped || self.sequence != 0 {
            return Err(VoiceRuntimeError::InvalidState);
        }
        self.validate_event_time(now_unix_ms)?;
        self.recognition
            .start(&self.session_id)
            .map_err(VoiceRuntimeError::Provider)?;
        self.state = VoiceSessionState::Listening;
        self.emit(now_unix_ms, VoiceConversationEventKind::ListeningStarted)
    }

    pub fn push_audio(
        &mut self,
        samples: &[i16],
        now_unix_ms: u64,
    ) -> Result<Vec<VoiceConversationEvent>, VoiceRuntimeError> {
        if self.stopped
            || !matches!(
                self.state,
                VoiceSessionState::Listening | VoiceSessionState::Speaking
            )
        {
            return Err(VoiceRuntimeError::InvalidState);
        }
        self.validate_event_time(now_unix_ms)?;
        self.vad.validate_frame(samples)?;
        self.recognition
            .push_audio(samples)
            .map_err(VoiceRuntimeError::Provider)?;
        let Some(activity) = self.vad.push_frame(samples)? else {
            return Ok(Vec::new());
        };
        let mut events = Vec::new();
        match activity {
            VadEvent::SpeechStarted => {
                if self.state == VoiceSessionState::Speaking {
                    self.synthesis.cancel();
                    self.state = VoiceSessionState::Interrupted;
                    events.push(self.emit(
                        now_unix_ms,
                        VoiceConversationEventKind::Interrupted {
                            reason: VoiceInterruptionReason::UserSpeech,
                        },
                    )?);
                }
                self.state = VoiceSessionState::Listening;
                events.push(self.emit(now_unix_ms, VoiceConversationEventKind::SpeechStarted)?);
            }
            VadEvent::SpeechEnded => {
                self.recognition
                    .finish_utterance()
                    .map_err(VoiceRuntimeError::Provider)?;
                events.push(self.emit(now_unix_ms, VoiceConversationEventKind::SpeechEnded)?);
                self.state = VoiceSessionState::Processing;
                events.push(self.emit(now_unix_ms, VoiceConversationEventKind::ProcessingStarted)?);
            }
        }
        Ok(events)
    }

    pub fn transcript(
        &mut self,
        text: impl Into<String>,
        final_result: bool,
        now_unix_ms: u64,
    ) -> Result<VoiceConversationEvent, VoiceRuntimeError> {
        if self.stopped {
            return Err(VoiceRuntimeError::InvalidState);
        }
        self.validate_event_time(now_unix_ms)?;
        let text = text.into();
        if text.trim().is_empty()
            || text.len() > MAX_PROVIDER_TEXT_BYTES
            || text.chars().any(char::is_control)
        {
            return Err(VoiceRuntimeError::InvalidText);
        }
        let event = if final_result {
            self.state = VoiceSessionState::Processing;
            VoiceConversationEventKind::TranscriptFinal { text }
        } else {
            VoiceConversationEventKind::TranscriptPartial { text }
        };
        self.emit(now_unix_ms, event)
    }

    pub fn speak(
        &mut self,
        text: &str,
        now_unix_ms: u64,
    ) -> Result<VoiceConversationEvent, VoiceRuntimeError> {
        if self.stopped {
            return Err(VoiceRuntimeError::InvalidState);
        }
        if text.trim().is_empty()
            || text.len() > MAX_PROVIDER_TEXT_BYTES
            || text.chars().any(char::is_control)
        {
            return Err(VoiceRuntimeError::InvalidText);
        }
        self.validate_event_time(now_unix_ms)?;
        self.synthesis
            .synthesize(text)
            .map_err(VoiceRuntimeError::Provider)?;
        self.vad.reset();
        self.state = VoiceSessionState::Speaking;
        self.emit(now_unix_ms, VoiceConversationEventKind::SpeakingStarted)
    }

    pub fn provider_failed(
        &mut self,
        error: VoiceProviderError,
        now_unix_ms: u64,
    ) -> Result<VoiceConversationEvent, VoiceRuntimeError> {
        if self.stopped {
            return Err(VoiceRuntimeError::InvalidState);
        }
        self.validate_event_time(now_unix_ms)?;
        let next_attempt = self.reconnect_attempt.saturating_add(1);
        if error.retryable {
            if let Some(delay_ms) = self.reconnect_policy.delay_ms(next_attempt) {
                self.reconnect_attempt = next_attempt;
                self.state = VoiceSessionState::Reconnecting;
                return self.emit(
                    now_unix_ms,
                    VoiceConversationEventKind::Reconnecting {
                        attempt: next_attempt,
                        delay_ms,
                    },
                );
            }
        }
        self.state = VoiceSessionState::Failed;
        self.emit(
            now_unix_ms,
            VoiceConversationEventKind::Failed {
                category: error.category,
            },
        )
    }

    pub fn reconnect(
        &mut self,
        now_unix_ms: u64,
    ) -> Result<VoiceConversationEvent, VoiceRuntimeError> {
        if self.stopped || self.state != VoiceSessionState::Reconnecting {
            return Err(VoiceRuntimeError::InvalidState);
        }
        self.validate_event_time(now_unix_ms)?;
        self.recognition
            .reconnect(&self.session_id)
            .map_err(VoiceRuntimeError::Provider)?;
        self.reconnect_attempt = 0;
        self.vad.reset();
        self.state = VoiceSessionState::Listening;
        self.emit(now_unix_ms, VoiceConversationEventKind::ListeningStarted)
    }

    pub fn stop(
        &mut self,
        reason: VoiceStopReason,
        now_unix_ms: u64,
    ) -> Result<VoiceConversationEvent, VoiceRuntimeError> {
        if self.stopped {
            return Err(VoiceRuntimeError::InvalidState);
        }
        self.validate_event_time(now_unix_ms)?;
        self.recognition.stop();
        self.synthesis.cancel();
        self.synthesis.stop();
        self.vad.reset();
        self.stopped = true;
        self.state = VoiceSessionState::Stopped;
        self.emit(now_unix_ms, VoiceConversationEventKind::Stopped { reason })
    }

    fn emit(
        &mut self,
        now_unix_ms: u64,
        event: VoiceConversationEventKind,
    ) -> Result<VoiceConversationEvent, VoiceRuntimeError> {
        self.validate_event_time(now_unix_ms)?;
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(VoiceRuntimeError::Protocol(ProtocolError::InvalidField(
                "voice.sequence",
            )))?;
        let value = VoiceConversationEvent {
            schema_version: VOICE_EVENT_SCHEMA_VERSION,
            session_id: self.session_id.clone(),
            sequence: self.sequence,
            occurred_at_unix_ms: now_unix_ms,
            event,
        };
        value.validate().map_err(VoiceRuntimeError::Protocol)?;
        self.last_event_at_unix_ms = now_unix_ms;
        Ok(value)
    }

    fn validate_event_time(&self, now_unix_ms: u64) -> Result<(), VoiceRuntimeError> {
        if now_unix_ms == 0 || now_unix_ms < self.last_event_at_unix_ms {
            return Err(VoiceRuntimeError::Protocol(ProtocolError::InvalidField(
                "voice.occurred_at_unix_ms",
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RecognitionFixture {
        starts: usize,
        frames: usize,
        finishes: usize,
        reconnects: usize,
        stops: usize,
    }

    impl RecognitionProvider for RecognitionFixture {
        fn start(&mut self, _: &str) -> Result<(), VoiceProviderError> {
            self.starts += 1;
            Ok(())
        }
        fn push_audio(&mut self, samples: &[i16]) -> Result<(), VoiceProviderError> {
            assert!(!samples.is_empty());
            self.frames += 1;
            Ok(())
        }
        fn finish_utterance(&mut self) -> Result<(), VoiceProviderError> {
            self.finishes += 1;
            Ok(())
        }
        fn reconnect(&mut self, _: &str) -> Result<(), VoiceProviderError> {
            self.reconnects += 1;
            Ok(())
        }
        fn stop(&mut self) {
            self.stops += 1;
        }
    }

    #[derive(Default)]
    struct SynthesisFixture {
        starts: usize,
        cancellations: usize,
        stops: usize,
    }

    impl SynthesisProvider for SynthesisFixture {
        fn synthesize(&mut self, text: &str) -> Result<(), VoiceProviderError> {
            assert!(!text.is_empty());
            self.starts += 1;
            Ok(())
        }
        fn cancel(&mut self) {
            self.cancellations += 1;
        }
        fn stop(&mut self) {
            self.stops += 1;
        }
    }

    fn coordinator() -> VoiceCoordinator<RecognitionFixture, SynthesisFixture> {
        VoiceCoordinator::new(
            RecognitionFixture::default(),
            SynthesisFixture::default(),
            "voice-session-1",
            VadConfig::default(),
            ReconnectPolicy::default(),
        )
        .unwrap()
    }

    #[test]
    fn vad_rejects_noise_and_echo_bursts_but_detects_bounded_speech() {
        let mut vad = VoiceActivityDetector::new(VadConfig::default()).unwrap();
        let quiet = vec![100; 480];
        let loud = vec![2_000; 480];
        for _ in 0..100 {
            assert_eq!(vad.push_frame(&quiet).unwrap(), None);
        }
        assert_eq!(vad.push_frame(&loud).unwrap(), None);
        assert_eq!(vad.push_frame(&quiet).unwrap(), None);
        assert_eq!(vad.push_frame(&loud).unwrap(), None);
        assert_eq!(vad.push_frame(&loud).unwrap(), None);
        assert_eq!(
            vad.push_frame(&loud).unwrap(),
            Some(VadEvent::SpeechStarted)
        );
        for _ in 0..7 {
            assert_eq!(vad.push_frame(&quiet).unwrap(), None);
        }
        assert_eq!(vad.push_frame(&quiet).unwrap(), Some(VadEvent::SpeechEnded));
    }

    #[test]
    fn barge_in_cancels_synthesis_and_returns_to_listening() {
        let mut coordinator = coordinator();
        coordinator.start(1_000).unwrap();
        coordinator.speak("hello", 1_001).unwrap();
        let loud = vec![2_000; 480];
        coordinator.push_audio(&loud, 1_002).unwrap();
        coordinator.push_audio(&loud, 1_003).unwrap();
        let events = coordinator.push_audio(&loud, 1_004).unwrap();
        assert_eq!(events.len(), 2);
        assert!(matches!(
            events[0].event,
            VoiceConversationEventKind::Interrupted {
                reason: VoiceInterruptionReason::UserSpeech
            }
        ));
        assert!(matches!(
            events[1].event,
            VoiceConversationEventKind::SpeechStarted
        ));
        assert_eq!(coordinator.state(), VoiceSessionState::Listening);
        assert_eq!(coordinator.synthesis.cancellations, 1);
    }

    #[test]
    fn reconnect_is_bounded_and_local_stop_prevents_resume() {
        let mut coordinator = coordinator();
        coordinator.start(1_000).unwrap();
        let error = VoiceProviderError {
            category: VoiceFailureCategory::NetworkUnavailable,
            retryable: true,
        };
        let event = coordinator.provider_failed(error, 1_001).unwrap();
        assert!(matches!(
            event.event,
            VoiceConversationEventKind::Reconnecting {
                attempt: 1,
                delay_ms: 250
            }
        ));
        coordinator.reconnect(1_002).unwrap();
        coordinator.stop(VoiceStopReason::UserStop, 1_003).unwrap();
        assert_eq!(
            coordinator.reconnect(1_004),
            Err(VoiceRuntimeError::InvalidState)
        );
        assert_eq!(coordinator.recognition.stops, 1);
        assert_eq!(coordinator.synthesis.stops, 1);
    }

    #[test]
    fn long_noisy_session_is_bounded_and_does_not_retain_audio() {
        let mut coordinator = coordinator();
        coordinator.start(1_000).unwrap();
        let quiet = vec![300; 480];
        for index in 0..50_000_u64 {
            let events = coordinator.push_audio(&quiet, 1_001 + index).unwrap();
            assert!(events.is_empty());
        }
        assert_eq!(coordinator.recognition.frames, 50_000);
        assert_eq!(coordinator.state(), VoiceSessionState::Listening);
        coordinator.stop(VoiceStopReason::UserStop, 51_001).unwrap();
    }

    #[test]
    fn transcript_and_event_sequence_are_strictly_bounded() {
        let mut coordinator = coordinator();
        let first = coordinator.start(1_000).unwrap();
        let partial = coordinator.transcript("hello", false, 1_001).unwrap();
        let final_result = coordinator.transcript("hello", true, 1_002).unwrap();
        partial.validate_after(&first).unwrap();
        final_result.validate_after(&partial).unwrap();
        assert_eq!(
            coordinator.transcript("", true, 1_003),
            Err(VoiceRuntimeError::InvalidText)
        );
    }

    #[test]
    fn invalid_input_is_rejected_before_provider_side_effects() {
        let mut coordinator = coordinator();
        assert!(coordinator.start(1_000).is_ok());

        assert_eq!(
            coordinator.push_audio(&[], 1_001),
            Err(VoiceRuntimeError::InvalidAudioFrame)
        );
        assert_eq!(coordinator.recognition.frames, 0);

        assert_eq!(
            coordinator.speak("hello", 999),
            Err(VoiceRuntimeError::Protocol(ProtocolError::InvalidField(
                "voice.occurred_at_unix_ms"
            )))
        );
        assert_eq!(coordinator.synthesis.starts, 0);
        assert_eq!(coordinator.state(), VoiceSessionState::Listening);
    }
}
