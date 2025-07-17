use crate::{Context, Error};
use poise::command;
use serenity::builder::{CreateEmbed, CreateMessage};
use serenity::model::prelude::*;

/// Skips the current track
#[command(prefix_command, slash_command, guild_only)]
pub async fn skip(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let guild_id = ctx.guild_id().unwrap();

    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        let _ = queue.skip();

        ctx.channel_id()
            .send_message(
                &ctx.serenity_context().http,
                CreateMessage::new().embed(
                    CreateEmbed::new()
                        .colour(0xffffff)
                        .title(":track_next: Skipped!")
                        .timestamp(Timestamp::now()),
                ),
            )
            .await?;
    } else {
        ctx.channel_id()
            .send_message(
                &ctx.serenity_context().http,
                CreateMessage::new().embed(
                    CreateEmbed::new()
                        .colour(0xf38ba8)
                        .title(":warning: Not in a voice channel.")
                        .timestamp(Timestamp::now()),
                ),
            )
            .await?;
    }
    ctx.reply("Skipped.").await?;
    Ok(())
}
