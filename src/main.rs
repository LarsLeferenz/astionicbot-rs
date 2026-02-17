mod commands;

use poise::{FrameworkError, serenity_prelude as serenity};
use serenity::all::ActivityData;
use serenity::{Client, GatewayIntents};
use songbird::SerenityInit;
use std::env;

type Error = serenity::Error;
type Context<'a> = poise::Context<'a, Data, Error>;

pub struct Data {
    http_client: reqwest::Client,
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
async fn main() {
    // Load .env if present (local dev). In production the container uses real env vars.
    dotenvy::dotenv().ok();

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

                match event {
                    serenity::FullEvent::Message { new_message } => {
                        //println!("Received message: {}", new_message.content);
                        if new_message.content.contains("Sup") {
                            new_message
                                .reply(&_ctx.http, "Not much")
                                .await
                                .expect("Das ist ja mies gelaufen");
                        }
                        let mut found = false;
                        new_message.sticker_items.iter().for_each(|sticker| {
                            println!("Sticker ID: {}, Name: {}", sticker.id, sticker.name);
                            if sticker.name == "Sup" {
                                found = true;
                            }
                        });
                        if found {
                            let audio_path = if std::path::Path::new("/app/grrr.mp3").exists() {
                                "/app/grrr.mp3"
                            } else {
                                "grrr.mp3"
                            };
                            let attachment = serenity::CreateAttachment::path(audio_path)
                                .await
                                .expect("Doof");
                            //new_message.reply(&_ctx.http, "Not much").await.expect("Mist");
                            new_message
                                .channel_id
                                .send_message(
                                    &_ctx.http,
                                    serenity::CreateMessage::new()
                                        .reference_message(new_message) // make it a reply (optional)
                                        .add_sticker_id(serenity::StickerId::new(
                                            1417970496720470126,
                                        ))
                                        .add_file(attachment),
                                )
                                .await
                                .expect("Mist");
                        }
                    }
                    _ => {}
                };

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
                        })
                })
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
