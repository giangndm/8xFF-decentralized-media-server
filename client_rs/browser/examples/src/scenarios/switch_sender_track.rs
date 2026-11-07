//! Sender switching: replace the active video source and verify remote frames.
use atm0s_media_client_browser::{get_user_media, BrowserRuntime, BrowserStream};
use atm0s_media_client_core::{ClientError, Command};
use wasm_bindgen::JsCast;

/// The caller owns the returned capture and must stop it during cleanup.
pub async fn switch(runtime: &mut BrowserRuntime) -> Result<BrowserStream, ClientError> {
    let replacement = get_user_media(false, true).await.map_err(|error| ClientError::Transport(format!("replacement capture: {error:?}")))?;
    let track = replacement
        .0
        .get_video_tracks()
        .get(0)
        .dyn_into::<web_sys::MediaStreamTrack>()
        .map_err(|_| ClientError::InvalidInput("replacement capture has no video track".into()))?;
    let source_id = track.id();
    runtime.register_track(source_id.clone(), track.clone());
    let result = replace(runtime, source_id).await;
    if let Err(error) = result {
        track.stop();
        return Err(error);
    }
    Ok(replacement)
}

async fn replace(runtime: &mut BrowserRuntime, source_id: String) -> Result<(), ClientError> {
    runtime.command(Command::DetachSender { name: "video_main".into() }).await?;
    runtime.command(Command::AttachSender { name: "video_main".into(), source_id }).await?;
    Ok(())
}
