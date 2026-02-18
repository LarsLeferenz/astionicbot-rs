use crate::commands::utils::to_time;
use crate::{Context, Error};
use poise::{CreateReply, command};
use serenity::builder::{CreateEmbed, CreateMessage};
use serenity::model::prelude::*;

/// Shows the currently playing track
#[command(prefix_command, slash_command, guild_only, aliases("np"))]
pub async fn nowplaying(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let guild_id = ctx.guild_id().unwrap();

    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();

        let current = match queue.current() {
            Some(current) => current,
            None => {
                ctx.send(
                    CreateReply::default().embed(
                        CreateEmbed::new()
                            .colour(0xf38ba8)
                            .title(":warning: Nothing is playing right now.")
                            .timestamp(Timestamp::now()),
                    ),
                )
                .await?;

                return Ok(());
            }
        };

        let track_info = current.get_info().await.unwrap();

        // Simplified version without metadata
        ctx.send(
            CreateReply::default().embed(
                CreateEmbed::new()
                    .colour(0xffffff)
                    .title("Now Playing")
                    .description("Track information is limited in this version.")
                    .fields(vec![
                        ("Position", to_time(track_info.position.as_secs()), true),
                        ("Status", format!("{:?}", track_info.playing), true),
                    ])
                    .timestamp(Timestamp::now()),
            ),
        )
        .await?;
    } else {
        ctx.send(
            CreateReply::default().embed(
                CreateEmbed::new()
                    .colour(0xf38ba8)
                    .title(":warning: Not in a voice channel.")
                    .timestamp(Timestamp::now()),
            ),
        )
        .await?;
    }
    Ok(())
}
