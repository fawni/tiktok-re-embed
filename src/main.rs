use miette::{miette, IntoDiagnostic, WrapErr};
use paris::{error, info, success};
use reqwest::redirect;
use serenity::{
    async_trait,
    http::{CacheHttp, Typing},
    model::prelude::*,
    prelude::*,
};

use tiktok_re_embed::tiktok::TikTok;

#[tokio::main]
async fn main() -> miette::Result<()> {
    kankyo::init().into_diagnostic().wrap_err("dotenv")?;
    let mut client = Client::builder(
        std::env::var("DISCORD_TOKEN")
            .into_diagnostic()
            .wrap_err("DISCORD_TOKEN")?,
        GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT,
    )
    .event_handler(Handler)
    .await
    .into_diagnostic()?;
    client.start().await.into_diagnostic()?;

    Ok(())
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        let bot = ready.user;
        success!("Ready! logged in as {}({})", bot.tag(), bot.id);
    }

    async fn message(&self, ctx: Context, msg: Message) {
        handle_message(ctx, msg)
            .await
            .unwrap_or_else(|err| error!("{err}"))
    }
}

async fn handle_message(ctx: Context, mut message: Message) -> miette::Result<()> {
    let re = TikTok::valid_urls();
    if !re[0].is_match(&message.content) && !re[1].is_match(&message.content) {
        return Ok(());
    };

    let client = reqwest::Client::builder()
        .redirect(redirect::Policy::custom(|attempt| attempt.stop()))
        .build()
        .into_diagnostic()?;

    let mut content = message.content.clone();
    if re[1].is_match(&content) {
        let url = &re[1].captures(&content).unwrap()[0];
        let res = client.get(url).send().await.into_diagnostic()?;
        content = res.headers()["location"]
            .to_str()
            .into_diagnostic()?
            .to_owned();
    }
    let aweme_id = &re[0].captures(&content).unwrap()[1];
    info!(
        "Re-embedding TikTok with aweme id {} | {}({})",
        aweme_id,
        message.author.tag(),
        message.author.id
    );

    let Ok(tiktok) = TikTok::from(aweme_id).await else {
        message
            .react(ctx.http(), ReactionType::Unicode(String::from("❌")))
            .await.into_diagnostic()?;

        return Err(miette!("Invalid TikTok ID"));
    };

    let file = client
        .get(tiktok.video_url)
        .send()
        .await
        .into_diagnostic()?
        .bytes()
        .await
        .into_diagnostic()?;

    let typing = Typing::start(ctx.http.clone(), message.channel_id.0).into_diagnostic()?;
    message
        .suppress_embeds(ctx.http())
        .await
        .into_diagnostic()?;
    message
        .channel_id
        .send_message(ctx.http(), |m| {
            m.add_file(AttachmentType::Bytes {
                data: file.as_ref().into(),
                filename: format!("{aweme_id}.mp4"),
            })
            .embed(|e| {
                e.author(|a| {
                    a.name(format!(
                        "{} (@{})",
                        tiktok.author.name, tiktok.author.username
                    ))
                    .url(format!("https://tiktok.com/@{}", tiktok.author.username))
                    .icon_url(tiktok.author.avatar_url())
                })
                .description(tiktok.description)
                .field("Likes", tiktok.statistics.likes, true)
                .field("Comments", tiktok.statistics.comments, true)
                .field("Views", tiktok.statistics.views, true)
                .color(0xF82054)
            })
            .reference_message(&message)
            .allowed_mentions(|am| am.empty_parse())
        })
        .await
        .into_diagnostic()?;
    _ = typing.stop();

    Ok(())
}
