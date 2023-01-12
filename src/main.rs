use anyhow::bail;
use paris::{error, info, success};
use reqwest::redirect;
use serenity::{
    async_trait,
    http::{CacheHttp, Typing},
    model::prelude::*,
    prelude::*,
};

mod tiktok;

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
        success!(
            "Ready! logged in as {}#{}({})",
            bot.name,
            bot.discriminator,
            bot.id
        );
    }

    async fn message(&self, ctx: Context, msg: Message) {
        handle_message(ctx, msg)
            .await
            .unwrap_or_else(|err| error!("{}", err))
    }
}

async fn handle_message(ctx: Context, mut msg: Message) -> anyhow::Result<()> {
    let regexes = tiktok::TikTok::valid_urls();
    let (re, mobile_re) = (&regexes[0], &regexes[1]);
    if !re.is_match(&msg.content) && !mobile_re.is_match(&msg.content) {
        return Ok(());
    };

    let client = reqwest::Client::builder()
        .redirect(redirect::Policy::custom(|attempt| attempt.stop()))
        .build()?;

    let mut content = msg.content.clone();
    if mobile_re.is_match(&content) {
        let mt = mobile_re.captures(&msg.content).unwrap().get(0).unwrap();
        let res = client.get(mt.as_str()).send().await?;
        let x = &res.text().await?;
        content = x.clone();
    }
    let aweme_id = re.captures(&content).unwrap().get(1).unwrap().as_str();
    info!(
        "Re-embedding TikTok with aweme id {} | {}({})",
        aweme_id,
        msg.author.tag(),
        msg.author.id
    );
    let tiktok = match tiktok::get_tiktok(aweme_id).await {
        Ok(v) => v,
        Err(_) => {
            msg.react(ctx.http(), ReactionType::Unicode(String::from("❌")))
                .await?;
            bail!("Failed to get TikTok!")
        }
    };
    let file = reqwest::get(tiktok.video_url).await?.bytes().await?;

    let typing = Typing::start(ctx.http.clone(), msg.channel_id.0)?;
    msg.suppress_embeds(ctx.http()).await?;
    msg.channel_id
        .send_message(ctx.http(), |r| {
            r.add_file(AttachmentType::Bytes {
                data: file.as_ref().into(),
                filename: String::from("tiktok.mp4"),
            })
            .embed(|e| {
                e.author(|a| {
                    a.name(format!(
                        "{} (@{})",
                        tiktok.author.name, tiktok.author.username
                    ))
                    .url(format!("https://tiktok.com/@{}", tiktok.author.username))
                    .icon_url(tiktok.author.avatar_url)
                })
                .description(tiktok.description)
                .field("Likes", tiktok.statistics.likes, true)
                .field("Comments", tiktok.statistics.comments, true)
                .field("Views", tiktok.statistics.views, true)
                .color(0xF82054)
            })
            .reference_message(&msg)
            .allowed_mentions(|am| am.empty_parse())
        })
        .await?;
    typing.stop().unwrap();

    Ok(())
}
