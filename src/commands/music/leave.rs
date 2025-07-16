use poise::{command, serenity_prelude as serenity, Context, CreateReply};
use serenity::builder::CreateEmbed;
use serenity::model::prelude::*;
use serenity::Error;

/// Leaves voice channel
#[command(slash_command, prefix_command, guild_only)]
pub async fn leave(ctx: Context<'_, (), Error>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();

    let manager = songbird::get(&ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let has_handler = manager.get(guild_id).is_some();

    if has_handler {
        if let Err(e) = manager.remove(guild_id).await {
            println!("Failed to leave voice channel: {}", e);
            ctx.send(CreateReply::default().embed(
                CreateEmbed::new()
                    .colour(0xf38ba8)
                    .title("Failed to leave voice channel.")
                    .timestamp(Timestamp::now())
            )).await?;
            return Ok(());
        }

        ctx.send(CreateReply::default().embed(
            CreateEmbed::new()
                .colour(0xffffff)
                .title("Left voice channel!")
                .timestamp(Timestamp::now())
        )).await?;
    } else {
        ctx.send(CreateReply::default().embed(
            CreateEmbed::new()
                .colour(0xf38ba8)
                .title(":warning: Not in a voice channel.")
                .timestamp(Timestamp::now())
        )).await?;
    }

    Ok(())
}
