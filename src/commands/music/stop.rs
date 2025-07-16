use serenity::model::prelude::*;
use poise::{command, Context};
use serenity::builder::{CreateEmbed, CreateMessage};
use serenity::Error;

/// Stops playback and clears the queue
#[command(prefix_command, slash_command, guild_only)]
pub async fn stop(ctx: Context<'_, (), Error>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();

    let manager = songbird::get(&ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        queue.stop();

        ctx.channel_id()
            .send_message(&ctx.serenity_context().http, CreateMessage::new()
                .embed(CreateEmbed::new()
                    .colour(0xffffff)
                    .title(":stop_button: Playlist stopped!")
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