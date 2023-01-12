use anyhow::bail;
use regex::Regex;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct TikTok {
    pub description: String,
    pub video_url: String,
    pub author: VideoAuthor,
    pub statistics: VideoStatistics,
}

impl TikTok {
    pub fn valid_urls() -> [Regex; 2] {
        [
            Regex::new(r"https?://(?:www\.|m\.)?tiktok\.com/(?:embed|@[\w\.-]+/video|v)/(\d+)")
                .unwrap(),
            Regex::new(r"https?://(?:vm|vt)\.tiktok\.com/(\w+)").unwrap(),
        ]
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct VideoAuthor {
    #[serde(rename = "nickname")]
    pub name: String,
    #[serde(rename = "unique_id")]
    pub username: String,
    pub avatar_uri: String,
    #[serde(skip)]
    pub avatar_url: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct VideoStatistics {
    #[serde(rename = "digg_count")]
    pub likes: u32,
    #[serde(rename = "comment_count")]
    pub comments: u32,
    #[serde(rename = "play_count")]
    pub views: u32,
}

#[derive(Deserialize, Debug, Clone)]
struct ApiResponse {
    aweme_list: Vec<Aweme>,
}

#[derive(Deserialize, Debug, Clone)]
struct Aweme {
    #[serde(rename = "aweme_id")]
    id: String,
    desc: String,
    author: VideoAuthor,
    video: ApiVideo,
    statistics: VideoStatistics,
}

#[derive(Deserialize, Debug, Clone)]
struct ApiVideo {
    play_addr: PlayAddr,
}

#[derive(Deserialize, Debug, Clone)]
struct PlayAddr {
    url_list: Vec<String>,
}

pub async fn get_tiktok(id: &str) -> anyhow::Result<TikTok> {
    let api_url = format!("https://api2.musical.ly/aweme/v1/feed/?aweme_id={}", id);
    let res = reqwest::get(api_url).await?.json::<ApiResponse>().await?;
    let mut aweme = res.aweme_list[0].clone();

    if aweme.id != id {
        bail!("TikTok not found!")
    }

    aweme.author.avatar_url = format!(
        "https://p16-amd-va.tiktokcdn.com/origin/{}.jpeg",
        aweme.author.avatar_uri
    );

    Ok(TikTok {
        video_url: aweme.video.play_addr.url_list[0].to_string(),
        description: aweme.desc,
        author: aweme.author,
        statistics: aweme.statistics,
    })
}
