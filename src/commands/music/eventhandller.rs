use serenity::async_trait;
use songbird::{Event, EventContext, EventHandler};
use tracing::{info, warn};

pub struct CustomSongbirdEventHandler;

impl CustomSongbirdEventHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl EventHandler for CustomSongbirdEventHandler {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        match ctx {
            EventContext::Track(track_events) => {
                for (state, handle) in *track_events {
                    match state.playing {
                        songbird::tracks::PlayMode::Play => {
                            info!("Track started: {:?}", handle.uuid());
                        }
                        songbird::tracks::PlayMode::End => {
                            info!(
                                "Track finished after {:.1}s (UUID: {:?})",
                                state.play_time.as_secs_f64(),
                                handle.uuid()
                            );
                        }
                        songbird::tracks::PlayMode::Pause => {
                            info!("Track paused: {:?}", handle.uuid());
                        }
                        songbird::tracks::PlayMode::Stop => {
                            info!("Track stopped: {:?}", handle.uuid());
                        }
                        songbird::tracks::PlayMode::Errored(ref error) => {
                            warn!("Track errored: {:?} (UUID: {:?})", error, handle.uuid());
                        }
                        _ => {}
                    }
                }
            }
            EventContext::DriverConnect(_) => {
                info!("Voice driver connected");
            }
            EventContext::DriverReconnect(_) => {
                warn!("Voice driver reconnecting...");
            }
            EventContext::DriverDisconnect(_) => {
                info!("Voice driver disconnected");
            }
            _ => {}
        }

        None
    }
}
