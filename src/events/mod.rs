mod message;
mod ready;

use crate::{Data, Error};
use poise::FrameworkContext;
use poise::serenity_prelude as serenity;

use serenity::Context;

pub trait HandleEvent {
    async fn handle(
        &self,
        ctx: &Context,
        framework: &FrameworkContext<'_, Data, Error>,
        data: &Data,
    ) -> Result<(), Error>;
}

impl HandleEvent for serenity::FullEvent {
    async fn handle(
        &self,
        ctx: &Context,
        framework: &FrameworkContext<'_, Data, Error>,
        data: &Data,
    ) -> Result<(), Error> {
        match self {
            serenity::FullEvent::Ready { data_about_bot } => {
                ready::handle_ready(ctx, framework, data, data_about_bot).await
            }
            serenity::FullEvent::Message { new_message } => {
                message::handle_message(ctx, framework, data, new_message).await
            }
            _ => Ok(()),
        }
    }
}
