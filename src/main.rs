mod commands;

use std::env;

use poise::{FrameworkError, serenity_prelude as serenity};
use serenity::all::ActivityData;
use serenity::{Client, GatewayIntents};
use songbird::SerenityInit;

type Error = serenity::Error;
type Context<'a> = poise::Context<'a, Data, Error>;

pub struct Data {}

async fn on_error(error: FrameworkError<'_, Data, Error>) {
    // This is our custom error handler
    // They are many errors that can occur, so we only handle the ones we want to customize
    // and forward the rest to the default handler
    match error {
        poise::FrameworkError::Setup { error, .. } => panic!("Failed to start bot: {:?}", error),
        poise::FrameworkError::Command { error, ctx, .. } => {
            println!("Error in command `{}`: {:?}", ctx.command().name, error,);
        }
        error => {
            if let Err(e) = poise::builtins::on_error(error).await {
                println!("Error while handling error: {}", e)
            }
        }
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().expect("Failed to load .env file.");

    let token = env::var("DISCORD_TOKEN").expect("Set your DISCORD_TOKEN environment variable!");

    // Initialize error tracing
    tracing_subscriber::fmt::init();

    let options: poise::FrameworkOptions<Data, Error> = poise::FrameworkOptions {
        commands: vec![
            commands::help::help(),
            commands::music::clear::clear(),
            commands::music::join::join(),
            commands::music::nowplaying::nowplaying(),
            commands::music::pause::pause(),
            commands::music::play::play(),
            commands::music::queue::queue(),
            commands::music::resume::resume(),
            commands::music::shuffle::shuffle(),
            commands::music::skip::skip(),
            commands::music::stop::stop(),
        ],
        prefix_options: poise::PrefixFrameworkOptions {
            prefix: Some(env::var("PREFIX").unwrap_or_else(|_| "!".to_string())),
            ..Default::default()
        },
        // The global error handler for all error cases that may occur
        on_error: |error| Box::pin(on_error(error)),
        // This code is run before every command
        pre_command: |ctx| {
            Box::pin(async move {
                println!("Executing command {}...", ctx.command().qualified_name);
            })
        },
        // This code is run after a command if it was successful (returned Ok)
        post_command: |ctx| {
            Box::pin(async move {
                println!("Executed command {}!", ctx.command().qualified_name);
            })
        },
        event_handler: |_ctx, event, _framework, _data| {
            Box::pin(async move {
                println!(
                    "Got an event in event handler: {:?}",
                    event.snake_case_name()
                );
                Ok(())
            })
        },
        ..Default::default()
    };

    let intents = GatewayIntents::non_privileged()
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_VOICE_STATES;

    let framework = poise::Framework::builder()
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                println!("Logged in as {}", _ready.user.name);
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {})
            })
        })
        .options(options)
        .build();

    let mut client = Client::builder(&token, intents)
        .framework(framework)
        .register_songbird()
        .activity(ActivityData::custom("Watching the stars..."))
        .await
        .expect("Err creating client");

    tokio::select! {
        result = client.start() => {
            if let Err(why) = result {
                println!("Client error: {:?}", why);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("Received Ctrl+C, shutting down.");

            // Get songbird manager to stop all music
            client.shard_manager.shutdown_all().await;
            println!("Bot shutdown complete.");
        }
    }
}
