use crate::{Context, Error};
use poise::{CreateReply, command};
use serenity::builder::CreateEmbed;
use serenity::model::prelude::*;

/// Stops playback and clears the queue
#[command(prefix_command, slash_command, guild_only)]
pub async fn stop(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let guild_id = ctx.guild_id().unwrap();

    let manager = await_timeout_or_return!(
        ctx,
        songbird::get(ctx.serenity_context()),
        10,
        "Timed out trying to access voice manager"
    )
    .expect("Songbird Voice client placed in at initialisation.")
    .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = await_timeout_or_return!(
            ctx,
            handler_lock.lock(),
            10,
            "Timed out trying to aquire voice handler"
        );

        let queue = handler.queue();
        queue.stop();

        drop(handler);

        let _ = await_timeout_or_return!(
            ctx,
            manager.leave(guild_id),
            10,
            "Timed out trying to leave voice chat"
        );

        let leave_result = await_timeout_or_return!(
            ctx,
            manager.remove(guild_id),
            10,
            "Timed out trying clean up music queue.."
        );

        if let Err(e) = leave_result {
            println!("Failed to leave voice channel: {}", e);
            ctx.send(
                CreateReply::default().embed(
                    CreateEmbed::new()
                        .colour(0xf38ba8)
                        .title("Failed to leave voice channel.")
                        .timestamp(Timestamp::now()),
                ),
            )
            .await?;
            return Ok(());
        }

        ctx.send(
            CreateReply::default().embed(
                CreateEmbed::new()
                    .colour(0xffffff)
                    .title(":stop_button: Stopped playback and dropped queue!")
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
