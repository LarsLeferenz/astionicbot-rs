use serenity::model::prelude::*;
use poise::{command, Context};
use serenity::builder::{CreateEmbed, CreateMessage};
use serenity::Error;

/// Resumes playback of the current track
#[command(prefix_command, slash_command, guild_only)]
pub async fn resume(ctx: Context<'_, (), Error>, _input: String) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();

    let manager = songbird::get(&ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        let _ = queue.resume();

        ctx.channel_id()
            .send_message(&ctx.serenity_context().http, CreateMessage::new()
                .embed(CreateEmbed::new()
                    .colour(0xffffff)
                    .title(":arrow_forward: Resumed!")
                    .timestamp(Timestamp::now())
                )
            )
            .await?;
    } else {
        ctx.channel_id()
            .send_message(&ctx.serenity_context().http, CreateMessage::new()
                .embed(CreateEmbed::new()
                    .colour(0xf38ba8)
                    .title(":warning: Not in a voice channel.")
                    .timestamp(Timestamp::now())
                )
            )
            .await?;
    }
    Ok(())
}