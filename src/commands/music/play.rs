use crate::{Context, Error};
use poise::{CreateReply, serenity_prelude as serenity};
use regex::Regex;
use serenity::all::CreateEmbed;
use serenity::futures::stream::FuturesUnordered;
use serenity::futures::StreamExt;
use serenity::model::prelude::*;
use songbird::input::{Compose, YoutubeDl};
use songbird::tracks::TrackHandle;
use tokio::process::Command;
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

    let reply = ctx.send(CreateReply::default().embed(
        CreateEmbed::new()
            .colour(0xffffff)
            .title(":notes: Fetching song(s)...")
            .description("Please wait...")
            .timestamp(Timestamp::now())
        )
        .ephemeral(false)
    ).await?;

    if let Some(handler_lock) = manager.get(ctx.guild_id().unwrap()) {

        let http_client = &ctx.data().http_client;
        let mut handler = handler_lock.lock().await;

        let _result = handler.deafen(true).await;

        // Handle YT Music by redirecting to youtube.com equivalent
        if url.clone().starts_with("http") && url.contains("music.") {
            let _ = url.replace("music.", "");
        }

        // search on youtube for video with given name and pick first from search result
        if !url.clone().starts_with("http") {
            let source = YoutubeDl::new_search(http_client.clone(), search);

            handler.enqueue(source.clone().into()).await;

            let metadata = source.clone().aux_metadata().await.unwrap();

            reply.edit(ctx, CreateReply::default().embed(
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
                        ("Songs queued", format!("{}", handler.queue().len()), true)
                    ])
                    .timestamp(Timestamp::now())
                )
                .ephemeral(false)
            ).await?;

            return Ok(());
        // handle playlist
        } else if url.contains("playlist") {
            // goal is to immediately queue and start playing first track while processing whole queue

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
                    Regex::new(r#""url": "(https://www.youtube.com/watch\?v=[A-Za-z0-9]{11})""#)
                        .unwrap();

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
                    queued.push((metadata.title.unwrap_or("<Missing>".to_string()),
                                 format!("{} - [Link]({})",metadata.artist.unwrap_or("<Missing>".to_string()), url.clone(), ), false)
                    );

                    reply.edit(
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
        // handle live stream
        } else if url.contains("live") {
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
            return Ok(());
        // handle direct link to a video
        } else {
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
            return Ok(());
        }
    }
    Ok(())
}
