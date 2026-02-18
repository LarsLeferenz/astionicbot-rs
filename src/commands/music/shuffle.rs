use crate::{Context, Error};
use poise::{CreateReply, command};
use rand::Rng;
use serenity::builder::{CreateEmbed, CreateMessage};
use serenity::model::prelude::*;

/// Shuffles the current queue
#[command(prefix_command, slash_command, guild_only)]
pub async fn shuffle(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let guild_id = ctx.guild_id().unwrap();

    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();

        queue.modify_queue(|queue| {
            // skip the first track on queue because it's being played
            fisher_yates_shuffle(queue.make_contiguous()[1..].as_mut(), &mut rand::rng())
        });

        ctx.send(
            CreateReply::default().embed(
                CreateEmbed::new()
                    .colour(0xffffff)
                    .title(":notes: Queue shuffled!")
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
    ctx.reply("Shuffled.").await?;
    Ok(())
}

fn fisher_yates_shuffle<T, R>(arr: &mut [T], mut rng: R)
where
    R: rand::RngCore + Sized,
{
    let mut index = arr.len();
    while index >= 2 {
        index -= 1;
        arr.swap(index, rng.random_range(0..(index + 1)));
    }
}
