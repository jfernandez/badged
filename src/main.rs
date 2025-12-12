//! Minimal polkit authentication agent with GTK4.

mod agent;
mod authority;
mod ui;

use agent::{
    AgentChannels, AuthComplete, AuthRequest, CancelRequest, PamMessage, PasswordNeeded,
    PasswordResponse, ShutdownRequest, UserCancel, UserChange,
};
use std::sync::mpsc;

const AGENT_PATH: &str = "/org/freedesktop/PolicyKit1/AuthenticationAgent";

fn main() {
    gtk4::init().expect("Failed to initialize GTK4");

    // Agent -> UI channels
    let (request_tx, request_rx) = mpsc::channel::<AuthRequest>();
    let (cancel_tx, cancel_rx) = mpsc::channel::<CancelRequest>();
    let (pam_msg_tx, pam_msg_rx) = mpsc::channel::<PamMessage>();
    let (password_needed_tx, password_needed_rx) = mpsc::channel::<PasswordNeeded>();
    let (auth_complete_tx, auth_complete_rx) = mpsc::channel::<AuthComplete>();

    // UI -> Agent channels
    let (password_tx, password_rx) = mpsc::channel::<PasswordResponse>();
    let (user_change_tx, user_change_rx) = mpsc::channel::<UserChange>();
    let (user_cancel_tx, user_cancel_rx) = mpsc::channel::<UserCancel>();
    let (shutdown_tx, shutdown_rx) = mpsc::channel::<ShutdownRequest>();

    let agent_channels = AgentChannels {
        request_tx,
        cancel_tx,
        pam_msg_tx,
        password_needed_tx,
        password_rx,
        auth_complete_tx,
        user_change_rx,
        user_cancel_rx,
        shutdown_rx,
    };

    std::thread::spawn(move || {
        if let Err(e) = agent::run_blocking(AGENT_PATH, agent_channels) {
            eprintln!("Agent error: {e:#}");
            std::process::exit(1);
        }
    });

    let ui_channels = ui::UiChannels {
        request_rx,
        cancel_rx,
        pam_msg_rx,
        password_needed_rx,
        auth_complete_rx,
        password_tx,
        user_change_tx,
        user_cancel_tx,
        shutdown_tx,
    };

    ui::run(ui_channels);
}
