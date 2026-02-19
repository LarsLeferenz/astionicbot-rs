use crate::commands::music::eventhandller::CustomSongbirdEventHandler;
use crate::{Context, Error};
use poise::{CreateReply, ReplyHandle, serenity_prelude as serenity};
use regex::Regex;
use reqwest::Client;
use serenity::all::CreateEmbed;
use serenity::model::prelude::*;
use songbird::input::{AuxMetadata, Compose, YoutubeDl};
use songbird::{Call, CoreEvent};
use tokio::process::Command;
use tokio::sync::MutexGuard;
use tracing::info;

fn get_ytdlp_args() -> Vec<String> {
    let args = vec![
        // Use the standard web client. bgutil-ytdlp-pot-provider supplies PO tokens
        // automatically via the YT_DLP_POT_BGUTIL_BASEURL env var, so this is now
        // the most stable and well-tested option.
        "--extractor-args".to_string(),
        "youtube:player_client=web".to_string(),
        "--js-runtimes=node".to_string(),
        "--remote-components=ejs:github".to_string(),
        // Prefer opus in webm (native Discord format), fall back gracefully
        "--format".to_string(),
        "bestaudio[ext=webm]/bestaudio[ext=m4a]/bestaudio/best".to_string(),
        "--no-playlist".to_string(),
        "--force-ipv4".to_string(),
        "--retries".to_string(),
        "3".to_string(),
        "--socket-timeout".to_string(),
        "30".to_string(),
    ];

    args
}

#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn play(
    ctx: Context<'_>,
    #[description = "Either a url to a video, playlist or a search term."] input: String,
) -> Result<(), Error> {
    ctx.defer().await?;

    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    // Auto-join the user's voice channel if the bot isn't already in one
    if manager.get(ctx.guild_id().unwrap()).is_none() {
        let channel_id = ctx
            .guild()
            .unwrap()
            .voice_states
            .get(&ctx.author().id)
            .and_then(|voice_state| voice_state.channel_id);

        let connect_to = match channel_id {
            Some(channel) => channel,
            None => {
                ctx.send(
                    CreateReply::default().embed(
                        CreateEmbed::new()
                            .colour(0xf38ba8)
                            .title(":warning: Join a voice channel first!")
                            .timestamp(Timestamp::now()),
                    ),
                )
                .await?;
                return Ok(());
            }
        };

        if let Err(_) = manager.join(ctx.guild_id().unwrap(), connect_to).await {
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

    let reply = ctx
        .send(
            CreateReply::default().embed(
                CreateEmbed::new()
                    .colour(0xffffff)
                    .title(":notes: Fetching song(s)...")
                    .description("Please wait...")
                    .timestamp(Timestamp::now()),
            ),
        )
        .await?;

    if let Some(handler_lock) = manager.get(ctx.guild_id().unwrap()) {
        let http_client = &ctx.data().http_client;
        let mut handler = handler_lock.lock().await;

        let _ = handler.deafen(true).await;

        handler.add_global_event(
            songbird::Event::Core(CoreEvent::DriverDisconnect),
            CustomSongbirdEventHandler::new(),
        );

        // Redirect YouTube Music links to regular YouTube
        let url = if input.starts_with("http") && input.contains("music.") {
            input.replace("music.", "")
        } else {
            input.clone()
        };

        if !url.starts_with("http") {
            handle_search(ctx, url, &reply, http_client, &mut handler).await?;
        } else if url.contains("playlist") {
            handle_playlist(ctx, url, &reply, http_client, &mut handler).await?;
        } else if url.contains("live") {
            handle_livestream(ctx, url, &reply, http_client, &mut handler).await?;
        } else {
            handle_direct_url(ctx, url, &reply, http_client, handler).await?;
        }
    }

    Ok(())
}

async fn handle_direct_url(
    ctx: Context<'_>,
    url: String,
    reply: &ReplyHandle<'_>,
    http_client: &Client,
    mut handler: MutexGuard<'_, Call>,
) -> Result<(), Error> {
    let mut source = YoutubeDl::new(http_client.clone(), url.clone()).user_args(get_ytdlp_args());

    let metadata = match source.aux_metadata().await {
        Ok(meta) => meta,
        Err(e) => {
            info!("Failed to fetch metadata for URL {}: {:?}", url, e);
            reply
                .edit(
                    ctx,
                    CreateReply::default().embed(
                        CreateEmbed::new()
                            .colour(0xf38ba8)
                            .title(":warning: Failed to fetch video!")
                            .description(format!(
                                "Could not access this video. It may be private, region-locked, or temporarily unavailable.\n\nError: {}",
                                e
                            ))
                            .timestamp(Timestamp::now()),
                    ),
                )
                .await?;
            return Ok(());
        }
    };

    handler.enqueue(source.into()).await;

    reply
        .edit(
            ctx,
            CreateReply::default().embed(
                CreateEmbed::new()
                    .colour(0xffffff)
                    .title(":notes: Added to queue!")
                    .thumbnail(metadata.thumbnail.clone().unwrap_or_else(|| {
                        "https://images.unsplash.com/photo-1611162616475-46b635cb6868".to_string()
                    }))
                    .description(format!(
                        "{} - {}\n{}",
                        metadata
                            .title
                            .clone()
                            .unwrap_or_else(|| "Unknown Title".to_string()),
                        metadata
                            .artist
                            .clone()
                            .unwrap_or_else(|| "Unknown Artist".to_string()),
                        if let Some(duration) = &metadata.duration {
                            format!(
                                "Duration: {}:{:02}",
                                duration.as_secs() / 60,
                                duration.as_secs() % 60
                            )
                        } else {
                            "Duration: Unknown".to_string()
                        }
                    ))
                    .timestamp(Timestamp::now()),
            ),
        )
        .await?;

    Ok(())
}

async fn handle_livestream(
    ctx: Context<'_>,
    url: String,
    reply: &ReplyHandle<'_>,
    http_client: &Client,
    handler: &mut MutexGuard<'_, Call>,
) -> Result<(), Error> {
    let mut source = YoutubeDl::new(http_client.clone(), url.clone()).user_args(get_ytdlp_args());

    let metadata = match source.aux_metadata().await {
        Ok(meta) => meta,
        Err(e) => {
            info!(
                "Failed to fetch livestream metadata for URL {}: {:?}",
                url, e
            );
            reply
                .edit(
                    ctx,
                    CreateReply::default().embed(
                        CreateEmbed::new()
                            .colour(0xf38ba8)
                            .title(":warning: Failed to access livestream!")
                            .description(format!(
                                "Could not access this stream. It may be offline, private, or region-locked.\n\nError: {}",
                                e
                            ))
                            .timestamp(Timestamp::now()),
                    ),
                )
                .await?;
            return Ok(());
        }
    };

    handler.enqueue(source.into()).await;

    reply
        .edit(
            ctx,
            CreateReply::default().embed(
                CreateEmbed::new()
                    .colour(0xffffff)
                    .title(":notes: Added to queue!")
                    .thumbnail(metadata.thumbnail.clone().unwrap_or_else(|| {
                        "https://images.unsplash.com/photo-1611162616475-46b635cb6868".to_string()
                    }))
                    .description(format!(
                        "{} - {}",
                        metadata
                            .title
                            .clone()
                            .unwrap_or_else(|| "Unknown Title".to_string()),
                        metadata
                            .artist
                            .clone()
                            .unwrap_or_else(|| "Unknown Artist".to_string()),
                    ))
                    .fields(vec![("Total playtime", "Live stream", true)])
                    .timestamp(Timestamp::now()),
            ),
        )
        .await?;

    Ok(())
}

async fn handle_playlist(
    ctx: Context<'_>,
    url: String,
    reply: &ReplyHandle<'_>,
    http_client: &Client,
    handler: &mut MutexGuard<'_, Call>,
) -> Result<(), Error> {
    let get_raw_list = Command::new("yt-dlp")
        .args([
            "-j",
            "--flat-playlist",
            "--extractor-args",
            "youtube:player_client=tv,mweb",
            &url,
        ])
        .output()
        .await;

    let raw_list = match get_raw_list {
        Ok(output) => String::from_utf8_lossy(&output.stdout).into_owned(),
        Err(e) => {
            info!("Failed to fetch playlist: {:?}", e);
            reply
                .edit(
                    ctx,
                    CreateReply::default().embed(
                        CreateEmbed::new()
                            .colour(0xf38ba8)
                            .title(":warning: Failed to fetch playlist!")
                            .description("Could not retrieve the playlist. Please check the URL and try again.")
                            .timestamp(Timestamp::now()),
                    ),
                )
                .await?;
            return Ok(());
        }
    };

    let re =
        Regex::new(r#""url": "(https://www\.youtube\.com/watch\?v=[A-Za-z0-9_-]{11})""#).unwrap();

    let urls: Vec<String> = re
        .captures_iter(&raw_list)
        .map(|cap| cap[1].to_string())
        .collect();

    if urls.is_empty() {
        reply
            .edit(
                ctx,
                CreateReply::default().embed(
                    CreateEmbed::new()
                        .colour(0xf38ba8)
                        .title(":warning: No tracks found in playlist!")
                        .timestamp(Timestamp::now()),
                ),
            )
            .await?;
        return Ok(());
    }

    let mut queued: Vec<(String, String, bool)> = Vec::new();

    for track_url in &urls {
        info!("Queueing --> {}", track_url);
        let mut source =
            YoutubeDl::new(http_client.clone(), track_url.clone()).user_args(get_ytdlp_args());

        // Best-effort metadata; don't abort the whole playlist on a single failure
        let (title, artist) = match source.aux_metadata().await {
            Ok(meta) => (
                meta.title.unwrap_or_else(|| "<Unknown>".to_string()),
                meta.artist.unwrap_or_else(|| "<Unknown>".to_string()),
            ),
            Err(e) => {
                info!("Could not fetch metadata for {}: {:?}", track_url, e);
                ("<Unknown>".to_string(), "<Unknown>".to_string())
            }
        };

        handler.enqueue(source.into()).await;

        queued.push((title, format!("{} - [Link]({})", artist, track_url), false));

        reply
            .edit(
                ctx,
                CreateReply::default().embed(
                    CreateEmbed::new()
                        .title(":page_facing_up: Queueing playlist:")
                        .fields(queued.clone())
                        .timestamp(Timestamp::now()),
                ),
            )
            .await?;
    }

    Ok(())
}

async fn handle_search(
    ctx: Context<'_>,
    search: String,
    reply: &ReplyHandle<'_>,
    http_client: &Client,
    handler: &mut MutexGuard<'_, Call>,
) -> Result<(), Error> {
    let mut source =
        YoutubeDl::new_search(http_client.clone(), search.clone()).user_args(get_ytdlp_args());

    let metadata = match source.aux_metadata().await {
        Ok(meta) => meta,
        Err(e) => {
            info!("Search failed for '{}': {:?}", search, e);
            reply
                .edit(
                    ctx,
                    CreateReply::default().embed(
                        CreateEmbed::new()
                            .colour(0xf38ba8)
                            .title(":warning: Search failed!")
                            .description(format!(
                                "Could not find a result for **{}**.\n\nError: {}",
                                search, e
                            ))
                            .timestamp(Timestamp::now()),
                    ),
                )
                .await?;
            return Ok(());
        }
    };

    handler.enqueue(source.into()).await;

    reply
        .edit(
            ctx,
            create_search_result_embed(metadata, handler.queue().len()),
        )
        .await?;

    Ok(())
}

fn create_search_result_embed(metadata: AuxMetadata, queue_length: usize) -> CreateReply {
    CreateReply::default()
        .embed(
            CreateEmbed::new()
                .colour(0xffffff)
                .title(":notes: Song added to the queue!")
                .thumbnail(metadata.thumbnail.clone().unwrap_or_else(|| {
                    "https://images.unsplash.com/photo-1611162616475-46b635cb6868".to_string()
                }))
                .description(format!(
                    "{} - {}",
                    metadata
                        .title
                        .clone()
                        .unwrap_or_else(|| "Unknown Title".to_string()),
                    metadata
                        .artist
                        .clone()
                        .unwrap_or_else(|| "Unknown Artist".to_string()),
                ))
                .fields(vec![("Songs queued", format!("{}", queue_length), true)])
                .timestamp(Timestamp::now()),
        )
        .ephemeral(false)
}
