use crate::commands::music::eventhandller::CustomSongbirdEventHandler;
use crate::{Context, Error};
use poise::{CreateReply, ReplyHandle, serenity_prelude as serenity};
use regex::Regex;
use reqwest::Client;
use serenity::all::CreateEmbed;
use serenity::model::prelude::*;
use songbird::input::{AuxMetadata, Compose, YoutubeDl};
use songbird::{Call, CoreEvent};
use std::env;
use tokio::process::Command;
use tokio::sync::MutexGuard;
use tracing::info;

fn get_ytdlp_args() -> Vec<String> {
    let aggressive_mode = env::var("YTDLP_AGGRESSIVE_MODE")
        .unwrap_or_else(|_| "true".to_string())
        .to_lowercase()
        == "true";

    let mut args = vec![
        "--user-agent".to_string(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string(),
        "--add-header".to_string(),
        "Referer:https://www.youtube.com/".to_string(),
        "--retries".to_string(),
        "3".to_string(),
        "--sleep-interval".to_string(),
        "1".to_string(),
        "--format".to_string(),
        "bestaudio/best".to_string(),
        "--ignore-errors".to_string(),
        "--force-ipv4".to_string(),
        "--socket-timeout".to_string(),
        "30".to_string(),
    ];

    if aggressive_mode {
        args.extend(vec![
            "--add-header".to_string(),
            "Origin:https://www.youtube.com".to_string(),
            "--add-header".to_string(),
            "Accept-Language:en-US,en;q=0.9".to_string(),
            "--add-header".to_string(),
            "Accept:text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8".to_string(),
            "--extractor-args".to_string(),
            "youtube:player_client=android".to_string(),
            "--retries".to_string(),
            "5".to_string(),
            "--sleep-interval".to_string(),
            "2".to_string(),
            "--retry-sleep".to_string(),
            "linear=1:5:10".to_string(),
            "--format".to_string(),
            "bestaudio[ext=m4a]/bestaudio/best".to_string(),
            "--no-check-certificates".to_string(),
            "--socket-timeout".to_string(),
            "60".to_string(),
        ]);
    }

    args
}

#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn play(
    ctx: Context<'_>,
    #[description = "Either a url to a video, playlist or a search term."] input: String,
) -> Result<(), Error> {
    ctx.defer().await?;

    let url = input.clone();

    let search = input.clone();

    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    // A separate !join is inconvenient, so bot joins with !play if not in voice channel
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

        let manager = songbird::get(ctx.serenity_context())
            .await
            .expect("Songbird Voice client placed in at initialisation.")
            .clone();

        let result = manager.join(ctx.guild_id().unwrap(), connect_to).await;

        if let Err(_channel) = result {
            ctx.send(
                CreateReply::default().embed(
                    CreateEmbed::new()
                        .title(":warning: error joining channel.")
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
            CreateReply::default()
                .embed(
                    CreateEmbed::new()
                        .colour(0xffffff)
                        .title(":notes: Fetching song(s)...")
                        .description("Please wait...")
                        .timestamp(Timestamp::now()),
                )
                .ephemeral(false),
        )
        .await?;

    if let Some(handler_lock) = manager.get(ctx.guild_id().unwrap()) {
        let http_client = &ctx.data().http_client;
        let mut handler = handler_lock.lock().await;

        let _result = handler.deafen(true).await;

        handler.add_global_event(
            songbird::Event::Core(CoreEvent::DriverDisconnect),
            CustomSongbirdEventHandler {},
        );

        // Handle YT Music by redirecting to youtube.com equivalent
        let url = if url.starts_with("http") && url.contains("music.") {
            url.replace("music.", "")
        } else {
            url
        };

        // search on youtube for video with given name and pick first from search result
        if !url.clone().starts_with("http") {
            handle_search(ctx, search, &reply, http_client, &mut handler).await?;
            return Ok(());
        // handle playlist
        } else if url.contains("playlist") {
            // goal is to immediately queue and start playing first track while processing whole queue
            handle_playlist(ctx, url, &reply, http_client, &mut handler).await?;
            return Ok(());
        // handle live stream
        } else if url.contains("live") {
            handle_livestream(ctx, url, &reply, http_client, &mut handler).await?;
            return Ok(());
        // handle direct link to a video
        } else {
            handle_direct_url(ctx, url, &reply, http_client, handler).await?;
            return Ok(());
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
    // Try primary method first
    let mut source = YoutubeDl::new(http_client.clone(), url.clone()).user_args(get_ytdlp_args());
    let mut metadata_result = source.clone().aux_metadata().await;

    // If primary method fails with 403, try fallback strategies
    if let Err(ref e) = metadata_result {
        if e.to_string().contains("403") || e.to_string().contains("Forbidden") {
            info!(
                "Primary method failed with 403, trying fallback strategies for URL: {}",
                url
            );

            // Strategy 1: Try with different extractor args (iOS client)
            let fallback_args = vec![
                "--extractor-args".to_string(),
                "youtube:player_client=ios".to_string(),
                "--user-agent".to_string(),
                "com.google.ios.youtube/19.09.3 (iPhone14,3; U; CPU iOS 15_6 like Mac OS X)"
                    .to_string(),
                "--retries".to_string(),
                "3".to_string(),
                "--sleep-interval".to_string(),
                "3".to_string(),
            ];

            source = YoutubeDl::new(http_client.clone(), url.clone()).user_args(fallback_args);
            metadata_result = source.clone().aux_metadata().await;

            // Strategy 2: If iOS fails, try web client without cookies
            if let Err(ref e2) = metadata_result {
                if e2.to_string().contains("403") {
                    info!("iOS fallback failed, trying web client without cookies");
                    let minimal_args = vec![
                        "--extractor-args".to_string(),
                        "youtube:player_client=web".to_string(),
                        "--no-cookies".to_string(),
                        "--retries".to_string(),
                        "2".to_string(),
                        "--sleep-interval".to_string(),
                        "5".to_string(),
                    ];

                    source =
                        YoutubeDl::new(http_client.clone(), url.clone()).user_args(minimal_args);
                    metadata_result = source.clone().aux_metadata().await;
                }
            }
        }
    }

    let metadata = match metadata_result {
        Ok(meta) => meta,
        Err(e) => {
            info!("All extraction methods failed for URL {}: {:?}", url, e);
            reply.edit(ctx, CreateReply::default().embed(
                CreateEmbed::new()
                    .colour(0xf38ba8)
                    .title(":warning: Failed to fetch video!")
                    .description(format!("Could not access the video after trying multiple methods. This might be due to:\n• YouTube blocking the request (403 error)\n• Video is private or unavailable\n• Geographic restrictions\n• Rate limiting\n\n**Try:**\n• Using a search term instead of direct URL\n• Waiting 10-15 minutes and trying again\n• Using a different video\n\nError: {}", e))
                    .timestamp(Timestamp::now())
            )).await?;
            return Ok(());
        }
    };

    let _song = handler.enqueue(source.clone().into()).await;

    reply.edit(ctx, CreateReply::default().embed(
        CreateEmbed::new()
            .colour(0xffffff)
            .title(":notes: Added to playlist!")
            .thumbnail(metadata.thumbnail.clone().unwrap_or_else(|| String::from("https://images.unsplash.com/photo-1611162616475-46b635cb6868?ixlib=rb-4.0.3")))
            .description(format!(
                "{} - {}",
                metadata.title.clone().unwrap_or_else(|| "Unknown Title".to_string()),
                metadata.artist.clone().unwrap_or_else(|| "Unknown Artist".to_string())
            ))
            .fields(vec![
                ("Songs queued", format!("{}", handler.queue().len()), true),
            ])
            .timestamp(Timestamp::now())
    )
    ).await?;
    Ok(())
}

async fn handle_livestream(
    ctx: Context<'_>,
    url: String,
    reply: &ReplyHandle<'_>,
    http_client: &Client,
    handler: &mut MutexGuard<'_, Call>,
) -> Result<(), Error> {
    let source = YoutubeDl::new(http_client.clone(), url.clone()).user_args(get_ytdlp_args());

    // Try to get metadata first to catch 403 errors early
    let metadata_result = source.clone().aux_metadata().await;
    let metadata = match metadata_result {
        Ok(meta) => meta,
        Err(e) => {
            info!(
                "Failed to fetch livestream metadata for URL {}: {:?}",
                url, e
            );
            reply.edit(ctx, CreateReply::default().embed(
                CreateEmbed::new()
                    .colour(0xf38ba8)
                    .title(":warning: Failed to access livestream!")
                    .description(format!("Could not access the livestream. This might be due to:\n• YouTube blocking the request (403 error)\n• Stream is offline or private\n• Geographic restrictions\n\nError: {}", e))
                    .timestamp(Timestamp::now())
            )).await?;
            return Ok(());
        }
    };

    let _song = handler.enqueue(source.clone().into()).await;

    reply.edit(ctx, CreateReply::default().embed(
        CreateEmbed::new()
            .colour(0xffffff)
            .title(":notes: Added to playlist!")
            .thumbnail(metadata.thumbnail.clone().unwrap_or_else(|| String::from("https://images.unsplash.com/photo-1611162616475-46b635cb6868?ixlib=rb-4.0.3")))
            .description(format!(
                "{} - {}",
                metadata.title.clone().unwrap_or_else(|| "Unknown Title".to_string()),
                metadata.artist.clone().unwrap_or_else(|| "Unknown Artist".to_string())
            ))
            .fields(vec![
                ("Songs queued", format!("{}", handler.queue().len()), true),
                ("Total playtime", "infinite".to_string(), true)
            ])
            .timestamp(Timestamp::now())
    )
    ).await?;
    Ok(())
}

async fn handle_playlist(
    ctx: Context<'_>,
    url: String,
    reply: &ReplyHandle<'_>,
    http_client: &Client,
    handler: &mut MutexGuard<'_, Call>,
) -> Result<(), Error> {
    if handler.queue().current().is_none() {
        info!("Current queue is empty, launching first track");
        let get_raw_list = Command::new("yt-dlp")
            .args(["-j", "--flat-playlist", &url])
            .output()
            .await;

        let raw_list = match get_raw_list {
            Ok(list) => String::from_utf8(list.stdout).unwrap(),
            Err(_) => String::from("Error!"),
        };

        let re =
            Regex::new(r#""url": "(https://www.youtube.com/watch\?v=[A-Za-z0-9]{11})""#).unwrap();

        let urls: Vec<String> = re
            .captures_iter(&raw_list)
            .map(|cap| cap[1].to_string())
            .collect();

        let clone_urls = urls.clone();

        let mut queued: Vec<(String, String, bool)> = Vec::new();

        for url in clone_urls {
            info!("Queueing --> {}", url);
            let source =
                YoutubeDl::new(http_client.clone(), url.clone()).user_args(get_ytdlp_args());
            handler.enqueue(source.clone().into()).await;

            let metadata = source.clone().aux_metadata().await.unwrap();
            queued.push((
                metadata.title.unwrap_or("<Missing>".to_string()),
                format!(
                    "{} - [Link]({})",
                    metadata.artist.unwrap_or("<Missing>".to_string()),
                    url.clone(),
                ),
                false,
            ));

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
    // Try primary search method first
    let mut source =
        YoutubeDl::new_search(http_client.clone(), search.clone()).user_args(get_ytdlp_args());
    let mut metadata_result = source.clone().aux_metadata().await;

    // If primary search fails with 403, try simpler approach
    if let Err(ref e) = metadata_result {
        if e.to_string().contains("403") || e.to_string().contains("Forbidden") {
            info!(
                "Primary search failed with 403, trying minimal search approach for: {}",
                search
            );

            let minimal_search_args = vec![
                "--user-agent".to_string(),
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36".to_string(),
                "--no-cookies".to_string(),
                "--retries".to_string(),
                "2".to_string(),
                "--sleep-interval".to_string(),
                "3".to_string(),
                "--ignore-errors".to_string(),
            ];

            source = YoutubeDl::new_search(http_client.clone(), search.clone())
                .user_args(minimal_search_args);
            metadata_result = source.clone().aux_metadata().await;
        }
    }

    let metadata = match metadata_result {
        Ok(meta) => meta,
        Err(e) => {
            info!("All search methods failed for '{}': {:?}", search, e);
            reply.edit(ctx, CreateReply::default().embed(
                CreateEmbed::new()
                    .colour(0xf38ba8)
                    .title(":warning: Search failed!")
                    .description(format!("Could not search for '{}' after trying multiple methods. This might be due to:\n• YouTube blocking search requests (403 error)\n• Rate limiting\n• Network issues\n\n**Try:**\n• Searching for a different or simpler term\n• Using a direct YouTube URL instead\n• Waiting 10-15 minutes and trying again\n\nError: {}", search, e))
                    .timestamp(Timestamp::now())
            )).await?;
            return Ok(());
        }
    };

    handler.enqueue(source.clone().into()).await;

    reply
        .edit(
            ctx,
            create_search_result_embed(metadata, handler.queue().len()),
        )
        .await?;
    Ok(())
}

fn create_search_result_embed(metadata: AuxMetadata, queue_length: usize) -> CreateReply {
    CreateReply::default().embed(
        CreateEmbed::new()
            .colour(0xffffff)
            .title(":notes: Song added to the queue!")
            .thumbnail(metadata.thumbnail.clone().unwrap_or_else(|| String::from("https://images.unsplash.com/photo-1611162616475-46b635cb6868?ixlib=rb-4.0.3")))
            .description(format!(
                "{} - {}",
                metadata.title.clone().unwrap(),
                metadata.artist.clone().unwrap()
            ))
            .fields(vec![
                ("Songs queued", format!("{}", queue_length), true)
            ])
            .timestamp(Timestamp::now())
    ).ephemeral(false)
}
