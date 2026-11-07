//! Automatic mixer source selection is configured when joining the room.
use atm0s_media_client_core::MixerConfig;
use media_server_protocol::protobuf::features::mixer::Mode;

pub fn config() -> MixerConfig {
    MixerConfig {
        mode: Mode::Auto as i32,
        outputs: 3,
        sources: vec![],
    }
}
