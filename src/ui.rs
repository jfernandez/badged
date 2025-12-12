//! Minimal GTK4 UI for the polkit authentication agent.

use gtk4::glib;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use crate::agent::{
    AuthComplete, AuthRequest, CancelRequest, PamMessage, PasswordNeeded, PasswordResponse,
    ShutdownRequest, UserCancel, UserChange,
};

/// Channels for UI communication with agent.
pub struct UiChannels {
    // From agent
    pub request_rx: mpsc::Receiver<AuthRequest>,
    pub cancel_rx: mpsc::Receiver<CancelRequest>,
    pub pam_msg_rx: mpsc::Receiver<PamMessage>,
    pub password_needed_rx: mpsc::Receiver<PasswordNeeded>,
    pub auth_complete_rx: mpsc::Receiver<AuthComplete>,
    // To agent
    pub password_tx: mpsc::Sender<PasswordResponse>,
    pub user_change_tx: mpsc::Sender<UserChange>,
    pub user_cancel_tx: mpsc::Sender<UserCancel>,
    pub shutdown_tx: mpsc::Sender<ShutdownRequest>,
}

const CSS: &str = r#"
.auth-header {
    font-size: 18px;
    font-weight: bold;
    margin-bottom: 4px;
}

.auth-message {
    font-size: 13px;
    opacity: 0.8;
    margin-bottom: 12px;
}

.fingerprint-frame {
    background-color: rgba(128, 128, 128, 0.1);
    border-radius: 12px;
    padding: 20px 40px;
    margin: 8px 0;
}

.fingerprint-label {
    font-size: 48px;
    margin-bottom: 8px;
}

.fingerprint-status {
    font-size: 13px;
}

.fingerprint-status.error {
    color: #c01c28;
}

.fingerprint-status.success {
    color: #26a269;
}

.separator-label {
    opacity: 0.6;
    font-size: 12px;
    margin: 8px 0;
}
"#;

/// Run the GTK4 UI event loop.
pub fn run(channels: UiChannels) {
    let app = gtk4::Application::builder()
        .application_id("org.freedesktop.PolicyKit1.AuthenticationAgent")
        .build();

    let channels = Rc::new(RefCell::new(Some(channels)));

    app.connect_startup(|_| {
        load_css();
    });

    app.connect_activate(move |app| {
        let (window, widgets) = build_window(app);

        if let Some(ch) = channels.borrow_mut().take() {
            setup_channels(window, widgets, ch);
        }
    });

    app.run_with_args::<&str>(&[]);
}

fn load_css() {
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(CSS);

    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().expect("Could not get default display"),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

struct Widgets {
    message_label: gtk4::Label,
    fingerprint_label: gtk4::Label,
    fingerprint_status: gtk4::Label,
    separator_label: gtk4::Label,
    user_dropdown: gtk4::DropDown,
    user_box: gtk4::Box,
    password_box: gtk4::Box,
    password_entry: gtk4::PasswordEntry,
    cancel_button: gtk4::Button,
    auth_button: gtk4::Button,
}

fn build_window(app: &gtk4::Application) -> (gtk4::Window, Widgets) {
    let window = gtk4::Window::builder()
        .application(app)
        .title("Authentication Required")
        .default_width(380)
        .resizable(false)
        .modal(true)
        .build();

    let main_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(8)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .build();

    // Header
    let header_label = gtk4::Label::builder()
        .label("Authentication Required")
        .halign(gtk4::Align::Center)
        .build();
    header_label.add_css_class("auth-header");

    // Action message
    let message_label = gtk4::Label::builder()
        .label("")
        .wrap(true)
        .halign(gtk4::Align::Center)
        .build();
    message_label.add_css_class("auth-message");

    // Fingerprint area frame
    let fingerprint_frame = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .halign(gtk4::Align::Center)
        .build();
    fingerprint_frame.add_css_class("fingerprint-frame");

    // Fingerprint emoji as icon (works everywhere)
    let fingerprint_label = gtk4::Label::builder()
        .label("🔐")
        .halign(gtk4::Align::Center)
        .build();
    fingerprint_label.add_css_class("fingerprint-label");

    // Fingerprint status label
    let fingerprint_status = gtk4::Label::builder()
        .label("Waiting for authentication...")
        .wrap(true)
        .halign(gtk4::Align::Center)
        .build();
    fingerprint_status.add_css_class("fingerprint-status");

    fingerprint_frame.append(&fingerprint_label);
    fingerprint_frame.append(&fingerprint_status);

    // Separator (hidden until password is needed)
    let separator_label = gtk4::Label::builder()
        .label("— or enter password —")
        .halign(gtk4::Align::Center)
        .visible(false)
        .build();
    separator_label.add_css_class("separator-label");

    // User selection (hidden if only one user)
    let user_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(12)
        .margin_top(8)
        .build();

    let user_label = gtk4::Label::builder()
        .label("User:")
        .width_chars(10)
        .xalign(0.0)
        .build();

    let user_dropdown = gtk4::DropDown::from_strings(&[]);
    user_dropdown.set_hexpand(true);

    user_box.append(&user_label);
    user_box.append(&user_dropdown);

    // Password entry (hidden until password is needed)
    let password_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(12)
        .margin_top(4)
        .visible(false)
        .build();

    let password_label = gtk4::Label::builder()
        .label("Password:")
        .width_chars(10)
        .xalign(0.0)
        .build();

    let password_entry = gtk4::PasswordEntry::builder()
        .placeholder_text("Enter password")
        .show_peek_icon(true)
        .sensitive(false)
        .hexpand(true)
        .build();

    password_box.append(&password_label);
    password_box.append(&password_entry);

    // Buttons
    let button_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(8)
        .halign(gtk4::Align::End)
        .margin_top(16)
        .build();

    let cancel_button = gtk4::Button::with_label("Cancel");

    let auth_button = gtk4::Button::with_label("Authenticate");
    auth_button.add_css_class("suggested-action");
    auth_button.set_sensitive(false);

    button_box.append(&cancel_button);
    button_box.append(&auth_button);

    // Assemble
    main_box.append(&header_label);
    main_box.append(&message_label);
    main_box.append(&fingerprint_frame);
    main_box.append(&separator_label);
    main_box.append(&user_box);
    main_box.append(&password_box);
    main_box.append(&button_box);

    window.set_child(Some(&main_box));

    let widgets = Widgets {
        message_label,
        fingerprint_label,
        fingerprint_status,
        separator_label,
        user_dropdown,
        user_box,
        password_box,
        password_entry,
        cancel_button,
        auth_button,
    };

    (window, widgets)
}

fn setup_channels(window: gtk4::Window, widgets: Widgets, channels: UiChannels) {
    let users: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let initializing: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

    let UiChannels {
        request_rx,
        cancel_rx,
        pam_msg_rx,
        password_needed_rx,
        auth_complete_rx,
        password_tx,
        user_change_tx,
        user_cancel_tx,
        shutdown_tx,
    } = channels;

    let Widgets {
        message_label,
        fingerprint_label,
        fingerprint_status,
        separator_label,
        user_dropdown,
        user_box,
        password_box,
        password_entry,
        cancel_button,
        auth_button,
    } = widgets;

    let password_tx = Rc::new(password_tx);
    let user_change_tx = Rc::new(user_change_tx);

    // Poll for auth requests - show dialog
    let window_clone = window.clone();
    let users_clone = users.clone();
    let initializing_clone = initializing.clone();
    let message_label_clone = message_label.clone();
    let fingerprint_label_clone = fingerprint_label.clone();
    let fingerprint_status_clone = fingerprint_status.clone();
    let separator_label_clone = separator_label.clone();
    let user_dropdown_clone = user_dropdown.clone();
    let user_box_clone = user_box.clone();
    let password_box_clone = password_box.clone();
    let password_entry_clone = password_entry.clone();
    let auth_button_clone = auth_button.clone();

    glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
        if let Ok(request) = request_rx.try_recv() {
            // Block dropdown change signal during setup
            *initializing_clone.borrow_mut() = true;

            *users_clone.borrow_mut() = request.users.clone();

            message_label_clone.set_label(&request.message);

            // Reset fingerprint state
            fingerprint_label_clone.set_label("🔐");
            fingerprint_status_clone.set_label("Waiting for authentication...");
            fingerprint_status_clone.remove_css_class("error");
            fingerprint_status_clone.remove_css_class("success");

            // Setup user dropdown
            let user_strs: Vec<&str> = request.users.iter().map(|s| s.as_str()).collect();
            let model = gtk4::StringList::new(&user_strs);
            user_dropdown_clone.set_model(Some(&model));
            user_dropdown_clone.set_selected(0);

            // Hide user selection if only one user
            user_box_clone.set_visible(request.users.len() > 1);

            // Hide password section until PAM asks for it
            separator_label_clone.set_visible(false);
            password_box_clone.set_visible(false);
            password_entry_clone.set_text("");
            password_entry_clone.set_sensitive(false);
            auth_button_clone.set_sensitive(false);

            *initializing_clone.borrow_mut() = false;

            window_clone.present();
        }
        glib::ControlFlow::Continue
    });

    // Poll for PAM info/error messages
    let fingerprint_status_clone = fingerprint_status.clone();
    let fingerprint_label_clone = fingerprint_label.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
        if let Ok(pam_msg) = pam_msg_rx.try_recv() {
            fingerprint_status_clone.set_label(&pam_msg.text);

            if pam_msg.is_error {
                fingerprint_status_clone.add_css_class("error");
                fingerprint_status_clone.remove_css_class("success");
                fingerprint_label_clone.set_label("❌");
            } else {
                fingerprint_status_clone.remove_css_class("error");
                // Check for success indicators in message
                let text_lower = pam_msg.text.to_lowercase();
                if text_lower.contains("success") || text_lower.contains("verified") {
                    fingerprint_status_clone.add_css_class("success");
                    fingerprint_label_clone.set_label("✅");
                } else {
                    fingerprint_label_clone.set_label("👆");
                }
            }
        }
        glib::ControlFlow::Continue
    });

    // Poll for password needed signal - show and enable password entry
    let separator_label_clone = separator_label.clone();
    let password_box_clone = password_box.clone();
    let password_entry_clone = password_entry.clone();
    let auth_button_clone = auth_button.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
        if password_needed_rx.try_recv().is_ok() {
            separator_label_clone.set_visible(true);
            password_box_clone.set_visible(true);
            password_entry_clone.set_sensitive(true);
            password_entry_clone.grab_focus();
            auth_button_clone.set_sensitive(true);
        }
        glib::ControlFlow::Continue
    });

    // Poll for auth complete - hide dialog
    let window_clone = window.clone();
    let password_entry_clone = password_entry.clone();
    let fingerprint_status_clone = fingerprint_status.clone();
    let fingerprint_label_clone = fingerprint_label.clone();
    let auth_button_clone = auth_button.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
        if let Ok(complete) = auth_complete_rx.try_recv() {
            if complete.success {
                fingerprint_label_clone.set_label("✅");
                fingerprint_status_clone.set_label("Authentication successful");
                fingerprint_status_clone.add_css_class("success");
            }

            password_entry_clone.set_text("");
            password_entry_clone.set_sensitive(false);
            auth_button_clone.set_sensitive(false);

            // Small delay before hiding for visual feedback
            let window_to_hide = window_clone.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
                gtk4::prelude::GtkWindowExt::set_focus(&window_to_hide, gtk4::Widget::NONE);
                window_to_hide.set_visible(false);
            });
        }
        glib::ControlFlow::Continue
    });

    // Poll for cancel requests
    let window_clone = window.clone();
    let password_entry_clone = password_entry.clone();
    let fingerprint_status_clone = fingerprint_status.clone();
    let auth_button_clone = auth_button.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
        if cancel_rx.try_recv().is_ok() {
            password_entry_clone.set_text("");
            password_entry_clone.set_sensitive(false);
            auth_button_clone.set_sensitive(false);
            fingerprint_status_clone.set_label("");
            gtk4::prelude::GtkWindowExt::set_focus(&window_clone, gtk4::Widget::NONE);
            window_clone.set_visible(false);
        }
        glib::ControlFlow::Continue
    });

    // User dropdown change - notify agent to restart helper
    let users_clone = users.clone();
    let initializing_clone = initializing.clone();
    let user_change_tx_clone = user_change_tx.clone();
    let separator_label_clone = separator_label.clone();
    let password_box_clone = password_box.clone();
    let password_entry_clone = password_entry.clone();
    let auth_button_clone = auth_button.clone();
    let fingerprint_status_clone = fingerprint_status.clone();
    let fingerprint_label_clone = fingerprint_label.clone();
    user_dropdown.connect_selected_notify(move |dropdown| {
        // Ignore changes during initial setup
        if *initializing_clone.borrow() {
            return;
        }

        let users_list = users_clone.borrow();
        let selected = dropdown.selected() as usize;
        if let Some(username) = users_list.get(selected) {
            // Reset UI state since we're restarting auth
            separator_label_clone.set_visible(false);
            password_box_clone.set_visible(false);
            password_entry_clone.set_text("");
            password_entry_clone.set_sensitive(false);
            auth_button_clone.set_sensitive(false);
            fingerprint_status_clone.set_label("Waiting for authentication...");
            fingerprint_label_clone.set_label("🔐");
            fingerprint_status_clone.remove_css_class("success");
            fingerprint_status_clone.remove_css_class("error");

            let _ = user_change_tx_clone.send(UserChange {
                username: username.clone(),
            });
        }
    });

    // Cancel button - notify agent and hide dialog
    let window_clone = window.clone();
    let user_cancel_tx = Rc::new(user_cancel_tx);
    let user_cancel_tx_clone = user_cancel_tx.clone();
    cancel_button.connect_clicked(move |_| {
        let _ = user_cancel_tx_clone.send(UserCancel);
        gtk4::prelude::GtkWindowExt::set_focus(&window_clone, gtk4::Widget::NONE);
        window_clone.set_visible(false);
    });

    // Auth button - send password
    let password_tx_clone = password_tx.clone();
    let password_entry_clone = password_entry.clone();
    let auth_button_clone = auth_button.clone();
    let fingerprint_status_clone = fingerprint_status.clone();
    auth_button.connect_clicked(move |_| {
        let password = password_entry_clone.text().to_string();
        let _ = password_tx_clone.send(PasswordResponse { password });

        // Disable while authenticating
        password_entry_clone.set_sensitive(false);
        auth_button_clone.set_sensitive(false);
        fingerprint_status_clone.set_label("Authenticating...");
    });

    // Enter key triggers auth
    let auth_button_clone = auth_button.clone();
    password_entry.connect_activate(move |_| {
        if auth_button_clone.is_sensitive() {
            auth_button_clone.emit_clicked();
        }
    });

    // Shutdown handler
    window.application().unwrap().connect_shutdown(move |_| {
        let _ = shutdown_tx.send(ShutdownRequest);
    });
}
