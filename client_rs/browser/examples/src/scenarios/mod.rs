//! Scenario identifiers used by the browser lab. Each scenario is selected by
//! the `?example=`/select value and drives the shared Rust BrowserRuntime.

pub mod audio_mixer_auto;
pub mod audio_mixer_manual;
pub mod echo_fast;
pub mod echo_lazy;
pub mod echo_restart_ice;
pub mod echo_simulcast;
pub mod multi_track_receivers;
pub mod switch_sender_track;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    EchoFast,
    EchoLazy,
    EchoSimulcast,
    RestartIce,
    SwitchSenderTrack,
    MultiTrackReceivers,
    AudioMixerAuto,
    AudioMixerManual,
}

impl Scenario {
    pub fn is_mixer(self) -> bool {
        matches!(self, Self::AudioMixerAuto | Self::AudioMixerManual)
    }

    pub const ALL: [Self; 8] = [
        Self::EchoFast,
        Self::EchoLazy,
        Self::EchoSimulcast,
        Self::RestartIce,
        Self::SwitchSenderTrack,
        Self::MultiTrackReceivers,
        Self::AudioMixerAuto,
        Self::AudioMixerManual,
    ];

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "echo_fast" => Self::EchoFast,
            "echo_lazy" => Self::EchoLazy,
            "echo_simulcast" => Self::EchoSimulcast,
            "echo_restart_ice" => Self::RestartIce,
            "switch_sender_track" => Self::SwitchSenderTrack,
            "multi_track_receivers" => Self::MultiTrackReceivers,
            "audio_mixer_auto" => Self::AudioMixerAuto,
            "audio_mixer_manual" => Self::AudioMixerManual,
            _ => return None,
        })
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EchoFast => "echo_fast",
            Self::EchoLazy => "echo_lazy",
            Self::EchoSimulcast => "echo_simulcast",
            Self::RestartIce => "echo_restart_ice",
            Self::SwitchSenderTrack => "switch_sender_track",
            Self::MultiTrackReceivers => "multi_track_receivers",
            Self::AudioMixerAuto => "audio_mixer_auto",
            Self::AudioMixerManual => "audio_mixer_manual",
        }
    }
}
