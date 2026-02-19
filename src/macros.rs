macro_rules! await_timeout_or_return {
    ($ctx:expr, $fut:expr, $secs:expr, $title:expr) => {{
        let __result = tokio::select! {
            v = $fut => Some(v),
            _ = tokio::time::sleep(std::time::Duration::from_secs($secs)) => None,
        };
        match __result {
            Some(v) => v,
            None => {
                $ctx.send(
                    poise::CreateReply::default().embed(
                        serenity::builder::CreateEmbed::new()
                            .colour(0xf38ba8)
                            .title($title)
                            .timestamp(serenity::model::prelude::Timestamp::now()),
                    ),
                )
                .await?;
                return Err(serenity::Error::Other($title));
            }
        }
    }};
}
