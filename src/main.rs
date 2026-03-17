//! Polkit authentication agent with GTK4.

mod listener;
mod ui;

use listener::{BadgedListener, SharedState};
use ui::UiChannels;

fn main() {
    gtk4::init().expect("Failed to initialize GTK4");

    let (event_tx, event_rx) = std::sync::mpsc::channel();
    let shared = SharedState::new(event_tx);

    // Create and register the polkit listener.
    let agent_listener = BadgedListener::new(shared.clone());
    let _handler = agent_listener
        .register_for_current_session()
        .expect("Failed to register polkit agent");
    eprintln!("[main] Polkit agent registered");

    // Run the GTK4 UI (blocks until app exits).
    ui::run(UiChannels { event_rx, shared });
}
