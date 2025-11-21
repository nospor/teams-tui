use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Chat {
    pub id: String,
    pub topic: Option<String>,
    #[serde(rename = "chatType")]
    pub chat_type: String,
    #[serde(rename = "lastUpdatedDateTime")]
    pub last_updated: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatsResponse {
    value: Vec<Chat>,
}

pub async fn get_chats(access_token: &str) -> Result<Vec<Chat>> {
    let client = reqwest::Client::new();
    let url = "https://graph.microsoft.com/v1.0/me/chats";

    let response = client
        .get(url)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await?;
        anyhow::bail!("Failed to get chats: {} - {}", status, text);
    }

    let chats_response = response.json::<ChatsResponse>().await?;
    Ok(chats_response.value)
}
