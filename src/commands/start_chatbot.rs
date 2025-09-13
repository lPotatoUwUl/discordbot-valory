use serenity::builder::CreateApplicationCommand;
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use serenity::prelude::*;
use std::path::Path;
use std::process::{Child, Command as StdCommand};
use tokio::sync::Mutex;
use once_cell::sync::Lazy;

/// Global async-safe storage for the running chatbot process
static CHATBOT_PROCESS: Lazy<Mutex<Option<Child>>> = Lazy::new(|| Mutex::new(None));

/// Helper: start Python chatbot process
fn start_ai_chatbot() -> Result<Child, String> {
    let venv_path = "./venv";
    let python_exe = if cfg!(windows) {
        format!("{}/Scripts/python.exe", venv_path)
    } else {
        format!("{}/bin/python", venv_path)
    };

    let chatbot_script = "./ai_chatbot.py";

    if !Path::new(chatbot_script).exists() {
        return Err("ai_chatbot.py not found.".into());
    }

    StdCommand::new(python_exe)
        .arg(chatbot_script)
        .spawn()
        .map_err(|e| format!("Failed to start chatbot: {}", e))
}

/// Register /run-chatbot command
pub fn register_commands(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("run-chatbot")
        .description("Start the AI chatbot and launch the local Python server.")
}

/// Register /stop-chatbot command
pub fn register_stop_commands(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("stop-chatbot")
        .description("Stop the AI chatbot and end the chat session.")
}

/// Handle /run-chatbot
pub async fn run_chatbot(ctx: &Context, command: &ApplicationCommandInteraction) {
    let user_id = command.user.id;

    let mut chatbot = CHATBOT_PROCESS.lock().await;
    if chatbot.is_some() {
        let _ = command.create_interaction_response(&ctx.http, |r| {
            r.kind(serenity::model::application::interaction::InteractionResponseType::ChannelMessageWithSource)
             .interaction_response_data(|msg| msg.content("⚠️ Chatbot is already running."))
        }).await;
        return;
    }

    match start_ai_chatbot() {
        Ok(child) => {
            *chatbot = Some(child);
            let _ = command.create_interaction_response(&ctx.http, |r| {
                r.kind(serenity::model::application::interaction::InteractionResponseType::ChannelMessageWithSource)
                 .interaction_response_data(|msg| msg.content("✅ Chatbot started successfully."))
            }).await;
        }
        Err(err) => {
            let _ = command.create_interaction_response(&ctx.http, |r| {
                r.kind(serenity::model::application::interaction::InteractionResponseType::ChannelMessageWithSource)
                 .interaction_response_data(|msg| msg.content(format!("❌ {}", err)))
            }).await;
        }
    }
}

/// Handle /stop-chatbot
pub async fn stop_chatbot(ctx: &Context, command: &ApplicationCommandInteraction) {
    let user_id = command.user.id;

    let mut chatbot = CHATBOT_PROCESS.lock().await;
    if let Some(mut child) = chatbot.take() {
        let _ = child.kill();
        let _ = child.wait();
        let _ = command.create_interaction_response(&ctx.http, |r| {
            r.kind(serenity::model::application::interaction::InteractionResponseType::ChannelMessageWithSource)
             .interaction_response_data(|msg| msg.content("🛑 Chatbot has been stopped."))
        }).await;
    } else {
        let _ = command.create_interaction_response(&ctx.http, |r| {
            r.kind(serenity::model::application::interaction::InteractionResponseType::ChannelMessageWithSource)
             .interaction_response_data(|msg| msg.content("⚠️ Chatbot is not running."))
        }).await;
    }
}
