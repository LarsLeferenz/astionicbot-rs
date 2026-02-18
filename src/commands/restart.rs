use crate::{Context, Error};

/// Attempts to restart the bot through a special exit code for the invoking script.
#[poise::command(prefix_command, track_edits, slash_command)]
pub async fn restart(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;
    let data = ctx.data();

    let reply = ctx.reply("Attempting restart").await?;
    let message = reply.into_message().await.unwrap();
    let reply_id = message.id;

    let channel_id = message.channel_id;

    std::fs::write(
        "restart_signal.txt",
        format!("{}\n{}", channel_id, reply_id),
    )
    .expect("Failed to write restart signal file.");

    data.restart_requested.cancel();

    Ok(())
}
