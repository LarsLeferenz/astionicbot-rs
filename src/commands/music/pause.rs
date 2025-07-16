use serenity::model::prelude::*;
use poise::{command, Context};
use serenity::builder::{CreateEmbed, CreateMessage};
use serenity::Error;

/// Pauses the currently playing track
#[command(prefix_command, slash_command, guild_only)]
pub async fn pause(ctx: Context<'_, (), Error>, _input: String) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();

    let manager = songbird::get(&ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        if let Err(e) = queue.pause() {
            println!("Failed to pause track: {}", e);
            ctx.channel_id()
                .send_message(&ctx.serenity_context().http, CreateMessage::new()
                    .embed(CreateEmbed::new()
                        .colour(0xf38ba8)
                        .title("Failed to pause track.")
                        .timestamp(Timestamp::now())
                    )
                )
                .await?;
            return Ok(());
        }

        ctx.channel_id()
            .send_message(&ctx.serenity_context().http, CreateMessage::new()
                .embed(CreateEmbed::new()
                    .colour(0xffffff)
                    .title(":pause_button: Paused!")
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