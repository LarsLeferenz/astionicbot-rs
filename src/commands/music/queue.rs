use crate::commands::utils::to_time;
use crate::{Context, Error};
use poise::command;
use serenity::builder::{CreateEmbed, CreateMessage};
use serenity::model::prelude::*;

/// Shows the current queue
#[command(prefix_command, slash_command, guild_only)]
pub async fn queue(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let guild_id = ctx.guild_id().unwrap();

    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();

        let _ = match queue.current() {
            Some(current) => current,
            None => {
                ctx.channel_id()
                    .send_message(
                        &ctx.serenity_context().http,
                        CreateMessage::new().embed(
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

        let mut desc = String::from("+ - + - + - + - + - + - + - + - + - +\n");
        let mut total_time = 0;

        // Simplified version that doesn't rely on metadata
        for (i, _song) in queue.current_queue().iter().enumerate() {
            desc.push_str(&format!("{}. Track {}\n", i + 1, i + 1));
            // We can't reliably get metadata, so we'll just use a placeholder duration
            total_time += 180; // Assume 3 minutes per song
        }

        ctx.channel_id()
            .send_message(
                &ctx.serenity_context().http,
                CreateMessage::new().embed(
                    CreateEmbed::new()
                        .colour(0xffffff)
                        .title(":notes: - Queue - :notes:")
                        .fields(vec![
                            ("Queue length", format!("{}", queue.len()), true),
                            ("Total time", to_time(total_time), true),
                        ])
                        .description(desc)
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
    Ok(())
}
