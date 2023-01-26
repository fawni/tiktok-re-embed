use anyhow::bail;
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
async fn main() -> anyhow::Result<()> {
    kankyo::init()?;
    let mut client = Client::builder(
        std::env::var("DISCORD_TOKEN")?,
        GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT,
    )
    .event_handler(Handler)
    .await?;
    client.start().await?;

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

async fn handle_message(ctx: Context, mut message: Message) -> anyhow::Result<()> {
    let re = TikTok::valid_urls();
    if !re[0].is_match(&message.content) && !re[1].is_match(&message.content) {
        return Ok(());
    };

    let client = reqwest::Client::builder()
        .redirect(redirect::Policy::custom(|attempt| attempt.stop()))
        .build()?;

    let mut content = message.content.clone();
    if re[1].is_match(&content) {
        let url = &re[1].captures(&content).unwrap()[0];
        let res = client.get(url).send().await?;
        content = res.headers()["location"].to_str()?.to_owned();
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
            .await?;
        bail!("Failed to get TikTok!")
    };

    let file = client.get(tiktok.video_url).send().await?.bytes().await?;

    let typing = Typing::start(ctx.http.clone(), message.channel_id.0)?;
    message.suppress_embeds(ctx.http()).await?;
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
        .await?;
    _ = typing.stop();

    Ok(())
}
