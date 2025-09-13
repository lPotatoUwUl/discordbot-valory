use serenity::async_trait;
use serenity::model::prelude::*;
use serenity::model::application::interaction::Interaction;
use serenity::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use mongodb::{Client as MongoClient, bson::doc};
use tokio::sync::Mutex;
use chrono::Utc;
use crate::db::user::{self, User, Conversation, get_user_collection, get_nickname_by_discord_id};

pub struct Handler {
    pub db_client: MongoClient,
    pub pending_nicknames: Arc<Mutex<HashMap<u64, String>>>,
}

impl Handler {
    /// Fetch nickname from DB
    pub async fn fetch_nickname(&self, discord_id: u64) -> Option<String> {
        let collection = get_user_collection(&self.db_client);
        let nickname = get_nickname_by_discord_id(&collection, discord_id).await;
        println!(
            "[DEBUG] Fetched nickname from DB for Discord ID {}: {:?}",
            discord_id, nickname
        );
        nickname
    }
}

// -------------------------
// Helper to clean AI response
// -------------------------
pub fn clean_ai_response(response: &str) -> String {
    let mut text = response.to_string();

    let re_role = regex::Regex::new(r"\[\w+\]\s*[!?,.\-–]*\s*").unwrap();
    text = re_role.replace_all(&text, "").to_string();

    let re_action = regex::Regex::new(r"!\s*[^.!?]*").unwrap();
    text = re_action.replace_all(&text, "").to_string();

    let re_leading_punct = regex::Regex::new(r"^[!?,.\-–\s]+").unwrap();
    text = re_leading_punct.replace_all(&text, "").to_string();

    let re_space = regex::Regex::new(r"\s{2,}").unwrap();
    text = re_space.replace_all(&text, " ").to_string();

    let re_newline = regex::Regex::new(r"\s*\n\s*").unwrap();
    text = re_newline.replace_all(&text, "\n").to_string();

    text.trim().to_string()
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }

        // Only process messages from the specified channel
        const CHAT_CHANNEL_ID: u64 = 1413865642053992459;
        if msg.channel_id.0 != CHAT_CHANNEL_ID {
            return;
        }

    let discord_id = msg.author.id.0;
    let collection = get_user_collection(&self.db_client);

        // Step 1: check DB
        let user_exists = collection
            .find_one(doc! {"discord_id": discord_id.to_string()}, None)
            .await
            .unwrap_or(None)
            .is_some();

        // Step 2: onboarding
        if !user_exists {
            if msg.content.starts_with("!start") {
                let _ = msg.channel_id.say(&ctx.http, "Welcome! Please reply with your desired bot nickname.").await;
            } else if msg.content.starts_with("!nickname ") {
                let nickname = msg.content[10..].trim().to_string();
                {
                    let mut pending = self.pending_nicknames.lock().await;
                    pending.insert(discord_id, nickname.clone());
                }
                println!("[LOG] Pending nickname stored: {}", nickname);
                let _ = msg.channel_id.say(&ctx.http, format!("You chose '{}'. Type !confirm to register.", nickname)).await;
            } else if msg.content.starts_with("!confirm") {
                let nickname_opt = {
                    let mut pending = self.pending_nicknames.lock().await;
                    pending.remove(&discord_id)
                };

                if let Some(nickname) = nickname_opt {
                    let user = User {
                        id: None,
                        discord_id: discord_id.to_string(),
                        nickname: nickname.clone(),
                        conversations: Vec::new(),
                    };
                    match collection.insert_one(user, None).await {
                        Ok(_) => {
                            println!("[LOG] User {} added to DB with nickname '{}'", discord_id, nickname);
                            let _ = msg.channel_id.say(&ctx.http, format!("You have been added as '{}'. You can now chat with the AI!", nickname)).await;
                        }
                        Err(e) => {
                            eprintln!("[ERROR] Failed to insert user: {:?}", e);
                            let _ = msg.channel_id.say(&ctx.http, "Failed to add to DB.").await;
                        }
                    }
                } else {
                    let _ = msg.channel_id.say(&ctx.http, "No pending nickname found. Use !nickname <your_nickname> first.").await;
                }
            } else {
                let _ = msg.channel_id.say(&ctx.http, "You must complete onboarding first! Use !start or /setup-bot.").await;
            }

            println!("[LOG] AI blocked for user {} because they are not confirmed.", discord_id);
            return;
        }

        // Step 3: fetch nickname from DB
        let nickname = match self.fetch_nickname(discord_id).await {
            Some(name) => name,
            None => {
                eprintln!("[ERROR] User {} exists in DB but nickname not found!", discord_id);
                let _ = msg.channel_id.say(&ctx.http, "Error: Could not fetch your nickname from DB.").await;
                return;
            }
        };

        let channel = msg.channel_id;
        let http = ctx.http.clone();
        let user_message = msg.content.clone();
        let nickname_clone = nickname.clone();
        let db_client = Arc::new(self.db_client.clone());

        // Step 4: spawn AI request and save conversation safely
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            // Check if chatbot server is reachable before sending message
            let server_online = match client.get("http://127.0.0.1:5005/healthcheck").send().await {
                Ok(resp) => resp.status().is_success(),
                Err(_) => false,
            };

            if !server_online {
                // let _ = channel.say(&http, "Chatbot server is offline. Please try again later.").await;
                return;
            }

            let payload = serde_json::json!({
                "message": user_message,
                "nickname": nickname_clone
            });

            match client.post("http://127.0.0.1:5005/chat")
                .json(&payload)
                .send()
                .await
            {
                Ok(resp) => match resp.text().await {
                    Ok(mut text) => {
                        text = text.trim().to_string();
                        if text.is_empty() {
                            text = "The chatbot returned nothing.".to_string();
                        }

                        // Clean AI response
                        text = crate::handler::clean_ai_response(&text);

                        println!("[LOG] AI response: {}", text);

                        // Save conversation
                        let collection = get_user_collection(&db_client);
                        let conversation = Conversation {
                            prompt: user_message.clone(),
                            response: text.clone(),
                            timestamp: Utc::now().timestamp(),
                        };

                        if let Err(e) = collection.update_one(
                            doc! {"discord_id": discord_id.to_string()},
                            doc! { "$push": { "conversations": mongodb::bson::to_bson(&conversation).unwrap() }} ,
                            None
                        ).await {
                            eprintln!("[ERROR] Failed to save conversation: {:?}", e);
                        }

                        if let Err(e) = channel.say(&http, text).await {
                            eprintln!("[ERROR] Failed to send AI response: {:?}", e);
                        }
                    }
                    Err(e) => {
                        eprintln!("[ERROR] Failed to read AI response text: {:?}", e);
                        let _ = channel.say(&http, "Failed to read chatbot response.").await;
                    }
                },
                Err(e) => {
                    eprintln!("[ERROR] Failed to call chatbot server: {:?}", e);
                    let _ = channel.say(&http, "Failed to reach chatbot server.").await;
                }
            }
        });
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            let command_name = command.data.name.as_str();

            match command_name {
                "setup-bot" => {
                    crate::commands::setup_bot::handle_setup_bot(&ctx, &command, &self.db_client).await;
                }
                "run-chatbot" => {
                    crate::commands::start_chatbot::run_chatbot(&ctx, &command).await;
                }
                "stop-chatbot" => {
                    crate::commands::start_chatbot::stop_chatbot(&ctx, &command).await;
                }
                _ => {}
            }
        }
    }
}
