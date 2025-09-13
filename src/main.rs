use serenity::prelude::*;
use serenity::model::application::command::Command;
use serenity::prelude::GatewayIntents;
use std::path::Path;
use std::env;
use std::sync::Arc;
use std::collections::HashMap;
use mongodb::{Client as MongoClient, options::ClientOptions};
use std::process::Command as StdCommand;
use tokio::sync::Mutex;

mod db;       // must come before `use db::...`
mod commands;
mod handler;

use crate::handler::Handler;
use crate::commands::start_chatbot; // so we can register chatbot commands

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();


    // Get bot token
    let token = env::var("DISCORD_TOKEN").expect("Expected DISCORD_TOKEN in the environment");

    // MongoDB setup
    let mongo_uri = env::var("MONGODB_URI").unwrap_or("mongodb://localhost:27017".to_string());
    let client_options = ClientOptions::parse(&mongo_uri)
        .await
        .expect("Failed to parse MongoDB URI");
    let db_client = MongoClient::with_options(client_options).expect("Failed to connect to MongoDB");

    // Setup handler
    let handler = Handler {
        db_client,
        pending_nicknames: Arc::new(Mutex::new(HashMap::new())),
    };

    let intents = GatewayIntents::all();
    let application_id: u64 = env::var("APPLICATION_ID")
        .expect("Expected APPLICATION_ID in environment")
        .parse()
        .expect("APPLICATION_ID must be a valid u64");

    let mut client = serenity::Client::builder(&token, intents)
        .event_handler(handler)
        .application_id(application_id)
        .await
        .expect("Error creating client");

    println!("[LOG] Bot is running...");

    // Register all slash commands in one place (only once)
    register_slash_commands(&client.cache_and_http.http).await;

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}

/// Central place for all slash command registration
async fn register_slash_commands(http: &serenity::http::Http) {
    // Replace with your target guild ID
    let guild_id: serenity::model::id::GuildId = serenity::model::id::GuildId(1413865613474140211); // <-- CHANGE THIS

    // Register /setup-bot
    let setup_bot = guild_id.create_application_command(http, |command| {
        command
            .name("setup-bot")
            .description("Register yourself with the bot and set your nickname")
            .create_option(|opt| {
                opt.name("nickname")
                    .description("Your desired bot nickname")
                    .kind(serenity::model::prelude::command::CommandOptionType::String)
                    .required(true)
            })
    })
    .await
    .expect("Failed to register /setup-bot");
    println!("[LOG] Registered guild command: /setup-bot");

    // Register /run-chatbot
    let run_chatbot = guild_id.create_application_command(http, |c| {
        start_chatbot::register_commands(c)
    })
    .await
    .expect("Failed to register /run-chatbot");
    println!("[LOG] Registered guild command: /run-chatbot");

    // Register /stop-chatbot
    let stop_chatbot = guild_id.create_application_command(http, |c| {
        start_chatbot::register_stop_commands(c)
    })
    .await
    .expect("Failed to register /stop-chatbot");
    println!("[LOG] Registered guild command: /stop-chatbot");

    println!("[LOG] All guild slash commands registered.");
}
