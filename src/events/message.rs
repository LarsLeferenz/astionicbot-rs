use crate::{Data, Error};
use poise::FrameworkContext;
use poise::serenity_prelude as serenity;

use serenity::Context;
use serenity::all::Message;

pub async fn handle_message(
    ctx: &Context,
    _framework: &FrameworkContext<'_, Data, Error>,
    _data: &Data,
    message: &Message,
) -> Result<(), Error> {
    //println!("Received message: {}", new_message.content);
    if message.content.contains("<@717769413457215528>") {
        let audio_path = if std::path::Path::new("/app/grrr.mp3").exists() {
            "/app/grrr.mp3"
        } else {
            "grrr.mp3"
        };
        let attachment = serenity::CreateAttachment::path(audio_path)
            .await
            .expect("Doof");
        //new_message.reply(&_ctx.http, "Not much").await.expect("Mist");
        message
            .channel_id
            .send_message(
                &ctx.http,
                serenity::CreateMessage::new()
                    .reference_message(message) // make it a reply (optional)
                    .add_file(attachment),
            )
            .await
            .expect("Mist");
    }
    Ok(())
}
