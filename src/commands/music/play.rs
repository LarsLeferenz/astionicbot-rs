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
        if url.clone().starts_with("http") && url.contains("music.") {
            let _ = url.replace("music.", "");
        }

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
    let source = YoutubeDl::new(http_client.clone(), url);
    let _song = handler.enqueue(source.clone().into()).await;

    let metadata = source.clone().aux_metadata().await.unwrap();
    reply.edit(ctx, CreateReply::default().embed(
        CreateEmbed::new()
            .colour(0xffffff)
            .title(":notes: Added to playlist!")
            .thumbnail(metadata.thumbnail.clone().unwrap_or_else(|| String::from("https://images.unsplash.com/photo-1611162616475-46b635cb6868?ixlib=rb-4.0.3")))
            .description(format!(
                "{} - {}",
                metadata.title.clone().unwrap(),
                metadata.artist.clone().unwrap()
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
    let source = YoutubeDl::new(http_client.clone(), url);
    let _song = handler.enqueue(source.clone().into()).await;

    let metadata = source.clone().aux_metadata().await.unwrap();

    reply.edit(ctx, CreateReply::default().embed(
        CreateEmbed::new()
            .colour(0xffffff)
            .title(":notes: Added to playlist!")
            .thumbnail(metadata.thumbnail.clone().unwrap_or_else(|| String::from("https://images.unsplash.com/photo-1611162616475-46b635cb6868?ixlib=rb-4.0.3")))
            .description(format!(
                "{} - {}",
                metadata.title.clone().unwrap(),
                metadata.artist.clone().unwrap()
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
            let source = YoutubeDl::new(http_client.clone(), url.clone());
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
    let source = YoutubeDl::new_search(http_client.clone(), search);

    handler.enqueue(source.clone().into()).await;

    let metadata = source.clone().aux_metadata().await.unwrap();

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
