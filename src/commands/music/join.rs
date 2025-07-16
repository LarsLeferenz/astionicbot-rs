use poise::{command, serenity_prelude as serenity, Context, CreateReply};
use serenity::builder::CreateEmbed;
use serenity::model::prelude::*;
use serenity::Error;

/// Joins voice channel
#[command(slash_command, prefix_command)]
pub async fn join(ctx: Context<'_, (), Error>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();
    
    // Get the user's voice channel by extracting the info we need before the async call
    let user_channel_id = {
        let guild = ctx.guild().unwrap();
        guild.voice_states.get(&ctx.author().id)
            .and_then(|voice_state| voice_state.channel_id)
    };

    let manager = songbird::get(&ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    // Check if bot is already in a voice channel
    if manager.get(guild_id).is_some() {
        ctx.send(CreateReply::default().embed(
            CreateEmbed::new()
                .colour(0xffffff)
                .title("Already in voice channel!")
                .timestamp(Timestamp::now())
        )).await?;
        return Ok(());
    }

    let connect_to = match user_channel_id {
        Some(channel) => channel,
        None => {
            ctx.send(CreateReply::default().embed(
                CreateEmbed::new()
                    .colour(0xf38ba8)
                    .title(":warning: Join a voice channel first!")
                    .timestamp(Timestamp::now())
            )).await?;
            return Ok(());
        }
    };

    let result = manager.join(guild_id, connect_to).await;

    if let Err(_channel) = result {
        ctx.send(CreateReply::default().embed(
            CreateEmbed::new()
                .colour(0xf38ba8)
                .title(":warning: error joining channel.")
                .description("Please ensure I have the correct permissions.")
                .timestamp(Timestamp::now())
        )).await?;
        return Ok(());
    }

    ctx.send(CreateReply::default().embed(
        CreateEmbed::new()
            .colour(0xffffff)
            .title("Joined voice channel!")
            .timestamp(Timestamp::now())
    )).await?;
    Ok(())
}
