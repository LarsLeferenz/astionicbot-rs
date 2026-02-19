use crate::{Data, Error};
use poise::FrameworkContext;
use poise::serenity_prelude as serenity;

use serenity::Context;

pub async fn handle_ready(
    ctx: &Context,
    _framework: &FrameworkContext<'_, Data, Error>,
    _data: &Data,
    _data_about_bot: &serenity::model::gateway::Ready,
) -> Result<(), Error> {
    if std::fs::exists("restart_signal.txt").unwrap() {
        let content = std::fs::read_to_string("restart_signal.txt").unwrap();
        let mut lines = content.lines();
        let channel_id = lines
            .next()
            .and_then(|line| line.parse::<u64>().ok())
            .map(|integer| serenity::ChannelId::new(integer))
            .expect("Failed to parse channel ID from restart signal file.");
        let message_id = lines
            .next()
            .and_then(|line| line.parse::<u64>().ok())
            .map(|integer| serenity::MessageId::new(integer))
            .expect("Failed to parse message ID from restart signal file.");
        let message = channel_id
            .message(&ctx.http, message_id)
            .await
            .expect("Failed to fetch message for restart signal.");

        message
            .reply(&ctx.http, "Sucessfully restarted!")
            .await
            .expect("Failed to reply to message for restart signal.");

        std::fs::remove_file("restart_signal.txt").expect("Failed to delete restart signal file.");
    }

    Ok(())
}
