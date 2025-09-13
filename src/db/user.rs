use mongodb::bson::{doc, oid::ObjectId};
use mongodb::Collection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Conversation {
    pub prompt: String,
    pub response: String,
    pub timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    pub discord_id: String,
    pub nickname: String,

    #[serde(default)]
    pub conversations: Vec<Conversation>,
}

/// Returns the Mongo collection for users
pub fn get_user_collection(client: &mongodb::Client) -> Collection<User> {
    client
        .database("discord_bot")
        .collection::<User>("users")
}

/// Fetch nickname by Discord ID
pub async fn get_nickname_by_discord_id(
    collection: &Collection<User>,
    discord_id: u64,
) -> Option<String> {
    match collection
        .find_one(doc! {"discord_id": discord_id.to_string()}, None)
        .await
    {
        Ok(Some(user)) => Some(user.nickname),
        Ok(None) => None,
        Err(e) => {
            eprintln!("[ERROR] Failed to fetch nickname: {:?}", e);
            None
        }
    }
}
