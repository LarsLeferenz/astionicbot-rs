use crate::commands::music::say::synthesize_audio;
use crate::{Data, Error};
use ::serenity::all::{EditAttachments, EditMessage};
use mistralrs::ChatCompletionChunkResponse;
use mistralrs::ChunkChoice;
use mistralrs::Delta;
use mistralrs::GgufModelBuilder;
use mistralrs::PagedAttentionMetaBuilder;
use mistralrs::Response;
use mistralrs::TextModelBuilder;
use poise::FrameworkContext;
use poise::serenity_prelude as serenity;

use ::serenity::all::GetMessages;
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
        return handle_mention(ctx, _framework, _data, message).await;
    }
    //     let audio_path = if std::path::Path::new("/app/grrr.mp3").exists() {
    //         "/app/grrr.mp3"
    //     } else {
    //         "grrr.mp3"
    //     };
    //     let attachment = serenity::CreateAttachment::path(audio_path)
    //         .await
    //         .expect("Doof");
    //     //new_message.reply(&_ctx.http, "Not much").await.expect("Mist");
    //     message
    //         .channel_id
    //         .send_message(
    //             &ctx.http,
    //             serenity::CreateMessage::new()
    //                 .reference_message(message) // make it a reply (optional)
    //                 .add_file(attachment),
    //         )
    //         .await
    //         .expect("Mist");
    // }
    Ok(())
}

const SYSTEM_PROMPT: &str =
    "Du bist ein Discord Bot namens \"Larsibot\" mit der ID <@717769413457215528>.\n
Beantworte Fragen auf Deutsch, Englisch ist eine böse Sprache.\n
Außerdem erhälst du vorherige Nachrichten in dem Kanal, diese haben die Rolle \"context\".
Am Anfang der Nachricht musst du den Nutzer für seine Nachricht loben, im Sinne von \"Das ist die klügste Frage die ich je gehört habe\", aber nicht in genau diesem Wortlaut.
Deine Antworten sollten kurz sein, maximal 5 Sätze.
Am Ende der Frage MUSST du den Nutzer kreativ beleidigen.
Verwende einige Emojis, wenn der Kontext es erlaubt.";

async fn handle_mention(
    ctx: &Context,
    _framework: &FrameworkContext<'_, Data, Error>,
    data: &Data,
    message: &Message,
) -> Result<(), Error> {
    // This is where you would implement the logic to handle the mention, such as sending a response or performing an action.
    let mut reply = message
        .channel_id
        .send_message(
            &ctx.http,
            serenity::CreateMessage::new()
                .content("-")
                .reference_message(message), // make it a reply (optional)
        )
        .await?;

    let _ = data.llm_activity_tx.send(());

    let mut guard = tokio::select! {
        v = data.llm_model.lock() => v,
        _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {
            reply.edit(&ctx.http,  EditMessage::new().content("Timed out trying to aquire model")).await?;
            return Err(Error::Other("Timed out trying to lock model"))
        },
    };

    let model = match guard.as_ref() {
        Some(model) => model,
        None => {
            reply
                .edit(&ctx.http, EditMessage::new().content("Loading model..."))
                .await?;
            // let model = GgufModelBuilder::new(
            //     "./models",                                 // local directory containing the GGUF
            //     vec!["Qwen3-4B-Instruct-2507-Q3_K_L.gguf"], // local GGUF filename(s)
            // )
            // //.with_device(mistralrs::Device::Cuda(mistralrs::CudaDevice::new(0)?))
            // .with_logging()
            // .with_paged_attn(|| PagedAttentionMetaBuilder::default().build());

            let model = TextModelBuilder::new("./models/Qwen3-30B-A3B-Instruct-2507-FP8")
                .with_logging()
                .with_paged_attn(|| PagedAttentionMetaBuilder::default().build());

            let model = model.unwrap();
            let model = model.build().await;

            let model = match model {
                Ok(m) => m,
                Err(e) => {
                    reply
                        .edit(
                            &ctx.http,
                            EditMessage::new().content(format!("Failed to load model: {}", e)),
                        )
                        .await?;
                    return Err(Error::Other("Failed to load model"));
                }
            };

            *guard = Some(model);
            guard.as_ref().unwrap()
        }
    };

    reply
        .edit(&ctx.http, EditMessage::new().content("."))
        .await?;

    // Get the past 10 messages in the channel
    let messages = message
        .channel_id
        .messages(&ctx.http, GetMessages::default().before(message).limit(10))
        .await?;

    let mut llm_messages = mistralrs::TextMessages::new()
        .add_message(mistralrs::TextMessageRole::System, SYSTEM_PROMPT);

    for msg in messages.into_iter().rev() {
        llm_messages = llm_messages.add_message(
            mistralrs::TextMessageRole::Custom("context".to_string()),
            format!("{}: {}", msg.author.name, msg.content),
        );
    }
    llm_messages = llm_messages.add_message(
        mistralrs::TextMessageRole::User,
        format!("<@{}> sagte: {}", message.author.id, message.content),
    );

    let mut response = "".to_string();

    let stream = model.stream_chat_request(llm_messages).await;
    if let Err(e) = stream {
        reply
            .edit(
                &ctx.http,
                EditMessage::new().content(format!("Failed to generate response: {}", e)),
            )
            .await?;
        return Err(Error::Other("Failed to generate response"));
    }
    let mut stream = stream.unwrap();
    while let Some(chunk) = stream.next().await {
        if let Response::Chunk(ChatCompletionChunkResponse { choices, .. }) = chunk {
            if let Some(ChunkChoice {
                delta:
                    Delta {
                        content: Some(content),
                        ..
                    },
                ..
            }) = choices.first()
            {
                response.push_str(content);
                reply
                    .edit(&ctx.http, EditMessage::new().content(&response))
                    .await?;
            };
        }
    }

    println!("Finished llm stream, creating TTS");

    let tts_result = synthesize_audio(&response).await;

    let output_path = match tts_result {
        Ok(path) => path,
        Err(e) => {
            return Err(Error::Other("Failed to generate TTS"));
        }
    };

    let attachment = serenity::CreateAttachment::path(output_path)
        .await
        .expect("Failed to create attachment for TTS audio");

    reply
        .edit(
            &ctx.http,
            EditMessage::new()
                .content(&response)
                .attachments(EditAttachments::new().add(attachment)),
        )
        .await?;

    Ok(())
}
