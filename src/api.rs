use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const GRAPH_API_BASE: &str = "https://graph.microsoft.com/v1.0";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMember {
    pub id: Option<String>,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Chat {
    pub id: String,
    pub topic: Option<String>,
    #[serde(rename = "chatType")]
    pub chat_type: String,
    #[serde(rename = "lastUpdatedDateTime")]
    pub last_updated: Option<String>,
    #[serde(skip)]
    pub members: Vec<ChatMember>,
    #[serde(skip)]
    pub cached_display_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub id: String,
    #[serde(rename = "createdDateTime")]
    pub created_date_time: String,
    pub from: Option<MessageFrom>,
    pub body: Option<MessageBody>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MessageFrom {
    pub user: Option<MessageUser>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MessageUser {
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MessageBody {
    pub content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatsResponse {
    value: Vec<Chat>,
}

#[derive(Debug, Deserialize)]
struct MembersResponse {
    value: Vec<ChatMember>,
}

#[derive(Debug, Deserialize)]
struct MessagesResponse {
    value: Vec<Message>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub id: String,
    #[serde(rename = "userPrincipalName")]
    pub user_principal_name: Option<String>,
}

fn get_profile_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .context("Could not find config directory")?;
    let app_dir = config_dir.join(crate::config::APP_DIR_NAME);
    fs::create_dir_all(&app_dir)?;
    Ok(app_dir.join("profile.json"))
}

fn save_profile(user: &User) -> Result<()> {
    let path = get_profile_path()?;
    let json = serde_json::to_string_pretty(user)?;
    fs::write(path, json)?;
    Ok(())
}

fn load_profile() -> Result<Option<User>> {
    let path = get_profile_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let json = fs::read_to_string(path)?;
    let user: User = serde_json::from_str(&json)?;
    Ok(Some(user))
}

pub async fn get_me(access_token: &str) -> Result<User> {
    // Try to load from cache first
    if let Ok(Some(user)) = load_profile() {
        return Ok(user);
    }

    let client = reqwest::Client::new();
    let url = format!("{}/me", GRAPH_API_BASE);

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await?;
        anyhow::bail!("Failed to get user profile: {} - {}", status, text);
    }

    let user = response.json::<User>().await?;
    
    // Save to cache
    if let Err(e) = save_profile(&user) {
        eprintln!("Warning: Failed to save profile cache: {}", e);
    }
    
    Ok(user)
}

fn abbreviate_name(full_name: &str) -> String {
    let parts: Vec<&str> = full_name.split_whitespace().collect();
    match parts.len() {
        0 => String::new(),
        1 => parts[0].to_string(),
        _ => {
            let first_name = parts[0];
            let last_initial = parts[parts.len() - 1].chars().next().unwrap_or('?');
            format!("{} {}", first_name, last_initial)
        }
    }
}

async fn get_chat_members(access_token: &str, chat_id: &str) -> Result<Vec<ChatMember>> {
    let client = reqwest::Client::new();
    let url = format!("{}/chats/{}/members", GRAPH_API_BASE, chat_id);

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;

    if !response.status().is_success() {
        // If we can't get members, return empty vec instead of failing
        return Ok(Vec::new());
    }

    let members_response = response.json::<MembersResponse>().await?;
    Ok(members_response.value)
}

pub async fn get_messages(access_token: &str, chat_id: &str) -> Result<Vec<Message>> {
    let client = reqwest::Client::new();
    let url = format!("{}/chats/{}/messages", GRAPH_API_BASE, chat_id);

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await?;
        anyhow::bail!("Failed to get messages: {} - {}", status, text);
    }

    let messages_response = response.json::<MessagesResponse>().await?;
    Ok(messages_response.value)
}

#[derive(Debug, Serialize)]
struct SendMessageRequest {
    body: SendMessageBody,
}

#[derive(Debug, Serialize)]
struct SendMessageBody {
    content: String,
}

pub async fn send_message(access_token: &str, chat_id: &str, content: &str) -> Result<()> {
    let client = reqwest::Client::new();
    let url = format!("{}/chats/{}/messages", GRAPH_API_BASE, chat_id);

    let request_body = SendMessageRequest {
        body: SendMessageBody {
            content: content.to_string(),
        },
    };

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await?;
        anyhow::bail!("Failed to send message: {} - {}", status, text);
    }

    Ok(())
}

pub async fn get_chats(access_token: &str) -> Result<(Vec<Chat>, Option<String>)> {
    let client = reqwest::Client::new();
    let url = format!("{}/me/chats", GRAPH_API_BASE);

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await?;
        anyhow::bail!("Failed to get chats: {} - {}", status, text);
    }

    let chats_response = response.json::<ChatsResponse>().await?;
    
    // Filter out meeting chats - only show oneOnOne and group chats
    let mut filtered_chats: Vec<Chat> = chats_response
        .value
        .into_iter()
        .filter(|chat| chat.chat_type == "oneOnOne" || chat.chat_type == "group")
        .collect();
    
    // Fetch members for each chat to get display names
    for chat in &mut filtered_chats {
        chat.members = get_chat_members(access_token, &chat.id).await.unwrap_or_default();
    }
    
    // Detect the current user by finding the member that appears most frequently in oneOnOne chats
    // This member is most likely the current user
    let mut current_user_name: Option<String> = None;
    
    let one_on_one_chats: Vec<&Chat> = filtered_chats.iter()
        .filter(|c| c.chat_type == "oneOnOne")
        .collect();
    
    if !one_on_one_chats.is_empty() {
        // Count how many times each member NAME appears
        let mut name_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        
        for chat in &one_on_one_chats {
            for member in &chat.members {
                if let Some(name) = &member.display_name {
                    *name_counts.entry(name.clone()).or_insert(0) += 1;
                }
            }
        }
        
        // Find the member that appears most frequently (should be the current user)
        if let Some((name, count)) = name_counts.iter()
            .max_by_key(|(_, count)| *count) {
            
            // Only consider it the current user if they appear in at least 2 chats
            // or if there's only 1 oneOnOne chat
            if *count >= 2 || one_on_one_chats.len() == 1 {
                current_user_name = Some(name.clone());
            }
        }
    }
    
    // Now filter out the current user from all chats by name
    if let Some(user_name) = &current_user_name {
        for chat in &mut filtered_chats {
            chat.members.retain(|m| {
                m.display_name.as_ref().map(|name| name != user_name).unwrap_or(true)
            });
        }
    }
    
    // Compute display names for all chats
    for chat in &mut filtered_chats {
        chat.cached_display_name = if chat.chat_type == "oneOnOne" {
            // For oneOnOne, use the first member's name
            chat.members.first()
                .and_then(|m| m.display_name.clone())
        } else if chat.chat_type == "group" {
            // For group, prefer topic, otherwise show member names
            if let Some(topic) = &chat.topic {
                if !topic.is_empty() {
                    Some(topic.clone())
                } else {
                    // Show up to 3 member names (abbreviated)
                    let names: Vec<String> = chat.members
                        .iter()
                        .filter_map(|m| m.display_name.as_ref().map(|n| abbreviate_name(n)))
                        .take(3)
                        .collect();
                    
                    if !names.is_empty() {
                        Some(names.join(", "))
                    } else {
                        Some("Unnamed Group".to_string())
                    }
                }
            } else {
                // No topic - show member names (abbreviated)
                let names: Vec<String> = chat.members
                    .iter()
                    .filter_map(|m| m.display_name.as_ref().map(|n| abbreviate_name(n)))
                    .take(3)
                    .collect();
                
                if !names.is_empty() {
                    Some(names.join(", "))
                } else {
                    Some("Unnamed Group".to_string())
                }
            }
        } else {
            Some("Unknown Chat".to_string())
        };
    }
    
    Ok((filtered_chats, current_user_name))
}
