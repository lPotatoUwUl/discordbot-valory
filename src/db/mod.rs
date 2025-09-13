pub mod user;
use mongodb::{Client as MongoClient, options::ClientOptions, bson::doc, Collection};
use crate::db::user::User;

pub fn get_user_collection(db_client: &MongoClient) -> Collection<User> {
    db_client.database("discord_bot").collection::<User>("users")
}