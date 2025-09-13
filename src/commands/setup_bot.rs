use serenity::model::prelude::*;
use serenity::model::application::interaction::{
    application_command::ApplicationCommandInteraction,
    InteractionResponseType,
};
use serenity::prelude::*;
use crate::db::{get_user_collection, user::User as DbUser};
use mongodb::bson::oid::ObjectId;

pub async fn handle_setup_bot(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    db_client: &mongodb::Client,
) {
    // Step 1: Get the user ID and optional nickname argument
    let user_id = command.user.id.0.to_string();

    // Extract nickname argument if provided
    let nickname_arg = command
        .data
        .options
        .get(0)
        .and_then(|opt| opt.value.as_ref())
        .and_then(|val| val.as_str())
        .map(|s| s.to_string());

    let users = get_user_collection(db_client);

    // Step 2: Check if user already exists
    let user_exists = match users
        .find_one(mongodb::bson::doc! {"discord_id": &user_id}, None)
        .await
    {
        Ok(opt) => opt.is_some(),
        Err(e) => {
            eprintln!("[ERROR] Failed to query user collection: {:?}", e);
            false
        }
    };

    if user_exists {
        println!("[LOG] User {} tried /setup-bot but is already registered", user_id);
        let _ = command
            .create_interaction_response(&ctx.http, |r| {
                r.kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|d| d.content("You are already registered!"))
            })
            .await;
        return;
    }

    // Step 3: If no nickname argument, ask user to provide one
    let nickname = match nickname_arg {
        Some(n) => n,
        None => {
            let _ = command
                .create_interaction_response(&ctx.http, |r| {
                    r.kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|d| {
                            d.content("Please provide your desired nickname as an argument, e.g., `/setup-bot nickname:YourNick`")
                        })
                })
                .await;
            return;
        }
    };

    // Step 4: Save the user to MongoDB
    let new_user = DbUser {
        id: None,                          // MongoDB will generate this
        discord_id: user_id.clone(),
        nickname: nickname.clone(),
        conversations: Vec::new(),         // Start empty
    };

    match users.insert_one(new_user, None).await {
        Ok(_) => {
            println!("[LOG] User {} added to DB with nickname '{}'", user_id, nickname);
            let _ = command
                .create_interaction_response(&ctx.http, |r| {
                    r.kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|d| {
                            d.content(format!(
                                "Welcome, {}! You are now registered and can chat with the AI.",
                                nickname
                            ))
                        })
                })
                .await;
        }
        Err(e) => {
            eprintln!("[ERROR] Failed to insert user: {:?}", e);
            let _ = command
                .create_interaction_response(&ctx.http, |r| {
                    r.kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|d| {
                            d.content("Failed to register. Please try again later.")
                        })
                })
                .await;
        }
    }
}
