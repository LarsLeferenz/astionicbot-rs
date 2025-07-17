use crate::{Context, Error};
use poise::{CreateReply, command, serenity_prelude as serenity};
use serenity::builder::CreateEmbed;
use serenity::model::prelude::*;

/// Joins voice channel
#[command(slash_command, prefix_command, guild_only)]
pub async fn join(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let guild_id = ctx.guild_id().unwrap();

    // Get the user's voice channel by extracting the info we need before the async call
    let user_channel_id = {
        let guild = ctx.guild().unwrap();
        guild
            .voice_states
            .get(&ctx.author().id)
            .and_then(|voice_state| voice_state.channel_id)
    };

    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    // Check if bot is already in a voice channel
    if manager.get(guild_id).is_some() {
        ctx.send(
            CreateReply::default().embed(
                CreateEmbed::new()
                    .colour(0xffffff)
                    .title("Already in voice channel!")
                    .timestamp(Timestamp::now()),
            ),
        )
        .await?;
        return Ok(());
    }

    let connect_to = match user_channel_id {
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

    let result = manager.join(guild_id, connect_to).await;

    if let Err(_channel) = result {
        ctx.send(
            CreateReply::default().embed(
                CreateEmbed::new()
                    .colour(0xf38ba8)
                    .title(":warning: error joining channel.")
                    .description("Please ensure I have the correct permissions.")
                    .timestamp(Timestamp::now()),
            ),
        )
        .await?;
        return Ok(());
    }

    let handler_lock = match manager.get(guild_id) {
        Some(handler) => handler,
        None => {
            return Ok(());
        },
    };

    let _result = handler_lock.lock().await.deafen(true).await;


    ctx.send(
        CreateReply::default().embed(
            CreateEmbed::new()
                .colour(0xffffff)
                .title("Joined voice channel!")
                .timestamp(Timestamp::now()),
        ),
    )
    .await?;
    Ok(())
}
