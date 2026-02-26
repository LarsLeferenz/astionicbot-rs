use crate::{Context, Error};
use ::serenity::all::CreateAttachment;
use piper_rs::synth::AudioOutputConfig;
use poise::{CreateReply, command, serenity_prelude as serenity};
use rand::Rng;
use serenity::builder::CreateEmbed;
use serenity::model::prelude::*;
use songbird::input::File as SongbirdFile;
use songbird::{Event, EventContext, EventHandler, TrackEvent};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use tokio::fs;
use tokio::sync::Mutex;
use tokio::task;
use tracing::{info, warn};

const DEFAULT_TTS_CONFIG_PATH: &str = "de_DE-lars.onnx.json";
const MAX_TTS_LENGTH: usize = usize::MAX;

static TTS_SYNTHEZISER: LazyLock<Arc<Mutex<Option<piper_rs::synth::PiperSpeechSynthesizer>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(None)));

fn is_emoji(ch: char) -> bool {
    let code = ch as u32;
    matches!(
        code,
        0x1F300..=0x1FAFF
            | 0x1F1E6..=0x1F1FF
            | 0x2600..=0x26FF
            | 0x2700..=0x27BF
            | 0xFE00..=0xFE0F
    )
}

fn filter_emojis(input: &str) -> String {
    input.chars().filter(|&ch| !is_emoji(ch)).collect()
}

#[derive(Clone)]
struct ResumeAndCleanup {
    handler_lock: Arc<Mutex<songbird::Call>>,
    resume: bool,
    file_path: PathBuf,
}

#[serenity::async_trait]
impl EventHandler for ResumeAndCleanup {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if matches!(ctx, EventContext::Track(_)) {
            if self.resume {
                let handler = self.handler_lock.lock().await;
                let queue = handler.queue();
                let _ = queue.resume();
            }

            if let Err(e) = fs::remove_file(&self.file_path).await {
                warn!("Failed to remove TTS temp file {:?}: {}", self.file_path, e);
            }
        }

        None
    }
}
pub async fn synthesize_audio(text: &str) -> Result<PathBuf, String> {
    let config_path_raw = PathBuf::from(
        env::var("TTS_CONFIG_PATH").unwrap_or_else(|_| DEFAULT_TTS_CONFIG_PATH.to_string()),
    );
    let config_path = if config_path_raw.is_relative() {
        let models_dir = PathBuf::from("models");
        if config_path_raw.starts_with(&models_dir) {
            config_path_raw
        } else {
            models_dir.join(config_path_raw)
        }
    } else {
        config_path_raw
    };

    if fs::metadata(&config_path).await.is_err() {
        return Err(format!(
            "TTS model not found at `{}`. Place your model files in `./models` and set `TTS_CONFIG_PATH` if needed.",
            config_path.display()
        ));
    }

    let output_dir = PathBuf::from("models/tts_cache");
    if let Err(e) = fs::create_dir_all(&output_dir).await {
        return Err(format!(
            "Could not create `models/tts_cache` directory: {}",
            e
        ));
    }

    let file_name = format!("astionic_tts_{}.wav", rand::rng().random::<u64>());
    let output_path = output_dir.join(file_name);

    let text_owned = text.to_string();
    let config_path_buf = config_path.clone();
    let output_path_clone = output_path.clone();
    let speaker_id = env::var("TTS_SPEAKER_ID").ok();

    if TTS_SYNTHEZISER.lock().await.is_none() {
        let model = piper_rs::from_config_path(Path::new(&config_path_buf))
            .map_err(|e| format!("Failed to load model: {}", e))?;

        if let Some(sid) = speaker_id {
            let sid = sid
                .parse::<i64>()
                .map_err(|_| "TTS_SPEAKER_ID must be a number".to_string())?;
            model.set_speaker(sid);
        }

        let synth = piper_rs::synth::PiperSpeechSynthesizer::new(model)
            .map_err(|e| format!("Failed to create synthesizer: {}", e))?;

        *TTS_SYNTHEZISER.lock().await = Some(synth);
    }

    let output_config = AudioOutputConfig {
        rate: Some(7u8),
        volume: None,
        pitch: None,
        appended_silence_ms: None,
    };

    let synth_lock = &*TTS_SYNTHEZISER;
    let synth_guard = synth_lock.lock().await;
    let synth = synth_guard
        .as_ref()
        .ok_or("Speech synthesizer not initialized".to_string())?;

    synth
        .synthesize_to_file(
            Path::new(&output_path_clone),
            text_owned,
            Some(output_config),
        )
        .map_err(|e| format!("Synthesis failed: {}", e))?;

    Ok(output_path)
}

/// Generates local TTS audio and plays it in voice
#[command(slash_command, prefix_command, guild_only)]
pub async fn say(
    ctx: Context<'_>,
    #[description = "Text to speak."] text: String,
) -> Result<(), Error> {
    ctx.defer().await?;

    let trimmed = text.trim();
    let filtered = filter_emojis(trimmed);
    let filtered_trimmed = filtered.trim();

    if filtered_trimmed.is_empty() {
        ctx.send(
            CreateReply::default().embed(
                CreateEmbed::new()
                    .colour(0xf38ba8)
                    .title(":warning: Please provide text to speak.")
                    .timestamp(Timestamp::now()),
            ),
        )
        .await?;
        return Ok(());
    }

    if filtered_trimmed.chars().count() > MAX_TTS_LENGTH {
        ctx.send(
            CreateReply::default().embed(
                CreateEmbed::new()
                    .colour(0xf38ba8)
                    .title(":warning: Message too long.")
                    .description(format!(
                        "Please keep the message under {} characters.",
                        MAX_TTS_LENGTH
                    ))
                    .timestamp(Timestamp::now()),
            ),
        )
        .await?;
        return Ok(());
    }

    println!("Generating TTS for text: {}", filtered_trimmed);

    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let guild_id = ctx.guild_id().unwrap();

    let user_channel_id = ctx
        .guild()
        .unwrap()
        .voice_states
        .get(&ctx.author().id)
        .and_then(|voice_state| voice_state.channel_id);

    if manager.get(guild_id).is_none() {
        if let Some(connect_to) = user_channel_id {
            if let Err(_) = manager.join(guild_id, connect_to).await {
                ctx.send(
                    CreateReply::default().embed(
                        CreateEmbed::new()
                            .colour(0xf38ba8)
                            .title(":warning: Error joining channel.")
                            .description("Please ensure I have the correct permissions.")
                            .timestamp(Timestamp::now()),
                    ),
                )
                .await?;
                return Ok(());
            }
        }
    }

    let handler_lock = if user_channel_id.is_some() {
        match manager.get(guild_id) {
            Some(handler) => Some(handler),
            None => return Ok(()),
        }
    } else {
        None
    };

    let tts_result = synthesize_audio(filtered_trimmed).await;
    let output_path = match tts_result {
        Ok(path) => {
            info!("TTS synthesis successful: {:?}", path);
            path
        }
        Err(e) => {
            warn!("TTS synthesis failed: {}", e);
            ctx.send(
                CreateReply::default().embed(
                    CreateEmbed::new()
                        .colour(0xf38ba8)
                        .title(":warning: TTS synthesis failed.")
                        .description(e)
                        .timestamp(Timestamp::now()),
                ),
            )
            .await?;
            return Ok(());
        }
    };

    if handler_lock.is_none() {
        ctx.send(CreateReply::default().attachment(CreateAttachment::path(output_path).await?))
            .await?;
        return Ok(());
    }

    let handler_lock = handler_lock.expect("Voice handler missing");

    let queue = {
        let handler = handler_lock.lock().await;
        handler.queue().clone()
    };

    let mut was_playing = false;
    if let Some(current) = queue.current() {
        if let Ok(info) = current.get_info().await {
            if matches!(info.playing, songbird::tracks::PlayMode::Play) {
                was_playing = true;
            }
        }
    }

    if was_playing {
        if let Err(e) = queue.pause() {
            warn!("Failed to pause current track: {}", e);
            ctx.send(
                CreateReply::default().embed(
                    CreateEmbed::new()
                        .colour(0xf38ba8)
                        .title(":warning: Failed to pause music.")
                        .timestamp(Timestamp::now()),
                ),
            )
            .await?;
            return Ok(());
        }
    }

    let tts_source = SongbirdFile::new(output_path.clone());
    let tts_handle = {
        let mut handler = handler_lock.lock().await;
        handler.play_input(tts_source.into())
    };

    let cleanup_handler = ResumeAndCleanup {
        handler_lock: handler_lock.clone(),
        resume: was_playing,
        file_path: output_path.clone(),
    };

    let _ = tts_handle.add_event(Event::Track(TrackEvent::End), cleanup_handler.clone());
    let _ = tts_handle.add_event(Event::Track(TrackEvent::Error), cleanup_handler);

    info!("TTS playback started: {:?}", output_path);

    ctx.send(CreateReply::default().attachment(CreateAttachment::path(output_path).await?))
        .await?;

    Ok(())
}
