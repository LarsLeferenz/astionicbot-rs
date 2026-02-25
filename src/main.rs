#[macro_use]
mod macros;
mod commands;
mod events;

use poise::{FrameworkError, serenity_prelude as serenity};
use serenity::all::ActivityData;
use serenity::{Client, GatewayIntents};
use songbird::SerenityInit;
use std::env;
use std::process::ExitCode;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::events::HandleEvent;

type Error = serenity::Error;
type Context<'a> = poise::Context<'a, Data, Error>;

pub struct Data {
    http_client: reqwest::Client,
    restart_requested: tokio_util::sync::CancellationToken,
    llm_model: Arc<Mutex<Option<mistralrs::Model>>>,
}

async fn on_error(error: FrameworkError<'_, Data, Error>) {
    // This is our custom error handler
    // They are many errors that can occur, so we only handle the ones we want to customize
    // and forward the rest to the default handler
    match error {
        FrameworkError::Setup { error, .. } => panic!("Failed to start bot: {:?}", error),
        FrameworkError::Command { error, ctx, .. } => {
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
async fn main() -> ExitCode {
    // Load .env if present (local dev). In production the container uses real env vars.
    dotenvy::dotenv().ok();

    let token = env::var("DISCORD_TOKEN").expect("Set your DISCORD_TOKEN environment variable!");

    // Initialize error tracing
    tracing_subscriber::fmt::init();

    let options: poise::FrameworkOptions<Data, Error> = poise::FrameworkOptions {
        commands: vec![
            commands::help::help(),
            commands::restart::restart(),
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
        event_handler: |ctx, event, framework, data| {
            Box::pin(async move {
                println!(
                    "Got an event in event handler: {:?}",
                    event.snake_case_name()
                );
                event.handle(ctx, &framework, data).await
            })
        },
        ..Default::default()
    };

    let intents = GatewayIntents::non_privileged()
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_VOICE_STATES;

    let restart_requested_token = tokio_util::sync::CancellationToken::new();
    let restart_requested_token_clone = restart_requested_token.clone();

    let framework = poise::Framework::builder()
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                println!("Logged in as {}", _ready.user.name);
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {
                    http_client: reqwest::Client::builder()
                        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                        .default_headers({
                            let mut headers = reqwest::header::HeaderMap::new();
                            headers.insert("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8".parse().unwrap());
                            headers.insert("Accept-Language", "en-US,en;q=0.5".parse().unwrap());
                            headers.insert("Accept-Encoding", "gzip, deflate, br".parse().unwrap());
                            headers.insert("DNT", "1".parse().unwrap());
                            headers.insert("Connection", "keep-alive".parse().unwrap());
                            headers.insert("Upgrade-Insecure-Requests", "1".parse().unwrap());
                            headers.insert("Sec-Fetch-Dest", "document".parse().unwrap());
                            headers.insert("Sec-Fetch-Mode", "navigate".parse().unwrap());
                            headers.insert("Sec-Fetch-Site", "none".parse().unwrap());
                            headers.insert("Sec-Fetch-User", "?1".parse().unwrap());
                            headers
                        })
                        .timeout(std::time::Duration::from_secs(30))
                        .build().unwrap_or_else(|_| {
                            panic!("Failed to create http client")
                        }),
                        restart_requested: restart_requested_token_clone,
                        llm_model: Arc::new(Mutex::new(None)),
                })
            })
        })
        .options(options)
        .build();

    let mut client = Client::builder(&token, intents)
        .framework(framework)
        .register_songbird()
        .activity(ActivityData::custom("🎶 Fixed Youtube Playback!"))
        .await
        .expect("Err creating client");

    let exit_code = tokio::select! {
        result = client.start() => {
            if let Err(why) = result {
                println!("Client error: {:?}", why);
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }

        }
        _ = restart_requested_token.cancelled() => {
            println!("Restart requested, shutting down.");

            // Get songbird manager to stop all music
            client.shard_manager.shutdown_all().await;
            println!("Bot shutdown complete.");

            ExitCode::from(42)
        }
        _ = tokio::signal::ctrl_c() => {
            println!("Received Ctrl+C, shutting down.");

            // Get songbird manager to stop all music
            client.shard_manager.shutdown_all().await;
            println!("Bot shutdown complete.");
            ExitCode::SUCCESS
        }
    };

    exit_code
}
