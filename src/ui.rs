//! GTK4 authentication dialog UI.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use gtk4::glib;
use gtk4::prelude::*;

use crate::listener::{SharedState, UiEvent};

pub struct UiChannels {
    pub event_rx: mpsc::Receiver<UiEvent>,
    pub shared: Rc<SharedState>,
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

/// Run the GTK4 UI event loop (blocking).
pub fn run(channels: UiChannels) {
    let app = gtk4::Application::builder()
        .application_id("org.freedesktop.badged.Agent")
        .flags(gtk4::gio::ApplicationFlags::NON_UNIQUE)
        .build();

    let channels = Rc::new(std::cell::RefCell::new(Some(channels)));

    let app_clone = app.clone();
    app.connect_startup(move |_| {
        load_css();
        app_clone.activate();
    });

    app.connect_activate(move |app| {
        let (window, widgets) = build_window(app);
        if let Some(ch) = channels.borrow_mut().take() {
            setup_ui(window, widgets, ch);
        }
    });

    let _hold = app.hold();
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
    user_box: gtk4::Box,
    user_dropdown: gtk4::DropDown,
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

    let header_label = gtk4::Label::builder()
        .label("Authentication Required")
        .halign(gtk4::Align::Center)
        .build();
    header_label.add_css_class("auth-header");

    let message_label = gtk4::Label::builder()
        .label("")
        .wrap(true)
        .halign(gtk4::Align::Center)
        .build();
    message_label.add_css_class("auth-message");

    let fingerprint_frame = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .halign(gtk4::Align::Center)
        .build();
    fingerprint_frame.add_css_class("fingerprint-frame");

    let fingerprint_label = gtk4::Label::builder()
        .label("🔐")
        .halign(gtk4::Align::Center)
        .build();
    fingerprint_label.add_css_class("fingerprint-label");

    let fingerprint_status = gtk4::Label::builder()
        .label("Waiting for authentication...")
        .wrap(true)
        .halign(gtk4::Align::Center)
        .build();
    fingerprint_status.add_css_class("fingerprint-status");

    fingerprint_frame.append(&fingerprint_label);
    fingerprint_frame.append(&fingerprint_status);

    let separator_label = gtk4::Label::builder()
        .label("— or enter password —")
        .halign(gtk4::Align::Center)
        .visible(false)
        .build();
    separator_label.add_css_class("separator-label");

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
        user_box,
        user_dropdown,
        password_box,
        password_entry,
        cancel_button,
        auth_button,
    };

    (window, widgets)
}

fn setup_ui(window: gtk4::Window, widgets: Widgets, channels: UiChannels) {
    let UiChannels { event_rx, shared } = channels;
    let users: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let initializing: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
    let current_request_id: Rc<RefCell<Option<u64>>> = Rc::new(RefCell::new(None));

    let Widgets {
        message_label,
        fingerprint_label,
        fingerprint_status,
        separator_label,
        user_box,
        user_dropdown,
        password_box,
        password_entry,
        cancel_button,
        auth_button,
    } = widgets;

    // Poll listener events every 50ms.
    let window_c = window.clone();
    let message_label_c = message_label.clone();
    let fingerprint_label_c = fingerprint_label.clone();
    let fingerprint_status_c = fingerprint_status.clone();
    let separator_label_c = separator_label.clone();
    let user_box_c = user_box.clone();
    let user_dropdown_c = user_dropdown.clone();
    let password_box_c = password_box.clone();
    let password_entry_c = password_entry.clone();
    let auth_button_c = auth_button.clone();
    let shared_events = Rc::clone(&shared);
    let users_c = users.clone();
    let initializing_c = initializing.clone();
    let current_request_id_c = current_request_id.clone();

    glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
        while let Ok(event) = event_rx.try_recv() {
            match event {
                UiEvent::ShowDialog {
                    request_id,
                    message,
                    users,
                } => {
                    eprintln!("[ui] ShowDialog: {message}");
                    *current_request_id_c.borrow_mut() = Some(request_id);
                    *initializing_c.borrow_mut() = true;
                    *users_c.borrow_mut() = users.clone();
                    message_label_c.set_label(&message);
                    fingerprint_label_c.set_label("🔐");
                    fingerprint_status_c.set_label("Waiting for authentication...");
                    fingerprint_status_c.remove_css_class("error");
                    fingerprint_status_c.remove_css_class("success");
                    let user_refs: Vec<&str> = users.iter().map(|user| user.as_str()).collect();
                    let user_model = gtk4::StringList::new(&user_refs);
                    user_dropdown_c.set_model(Some(&user_model));
                    user_dropdown_c.set_selected(0);
                    separator_label_c.set_visible(false);
                    password_box_c.set_visible(false);
                    password_entry_c.set_text("");
                    password_entry_c.set_sensitive(false);
                    auth_button_c.set_sensitive(false);
                    user_box_c.set_visible(users.len() > 1);
                    *initializing_c.borrow_mut() = false;
                    window_c.present();
                }
                UiEvent::PamInfo(text) => {
                    eprintln!("[ui] PamInfo: {text}");
                    fingerprint_status_c.set_label(&text);
                    fingerprint_label_c.set_label("👆");
                    fingerprint_status_c.remove_css_class("error");
                    fingerprint_status_c.remove_css_class("success");
                }
                UiEvent::PamError(text) => {
                    eprintln!("[ui] PamError: {text}");
                    fingerprint_status_c.set_label(&text);
                    fingerprint_label_c.set_label("❌");
                    fingerprint_status_c.add_css_class("error");
                    fingerprint_status_c.remove_css_class("success");
                }
                UiEvent::PasswordNeeded => {
                    eprintln!("[ui] PasswordNeeded");
                    separator_label_c.set_visible(true);
                    password_box_c.set_visible(true);
                    password_entry_c.set_sensitive(true);
                    password_entry_c.grab_focus();
                    auth_button_c.set_sensitive(true);
                }
                UiEvent::AuthComplete { success } => {
                    eprintln!("[ui] AuthComplete: {success}");
                    password_entry_c.set_text("");
                    password_entry_c.set_sensitive(false);
                    auth_button_c.set_sensitive(false);
                    if success {
                        fingerprint_label_c.set_label("✅");
                        fingerprint_status_c.set_label("Authentication successful");
                        fingerprint_status_c.add_css_class("success");
                        let win = window_c.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(300),
                            move || win.set_visible(false),
                        );
                    } else {
                        window_c.set_visible(false);
                    }
                    *current_request_id_c.borrow_mut() = None;
                }
                UiEvent::PolkitCancelled { request_id } => {
                    if Some(request_id) == *current_request_id_c.borrow()
                        && shared_events.cancel_request(request_id)
                    {
                        password_entry_c.set_text("");
                        password_entry_c.set_sensitive(false);
                        auth_button_c.set_sensitive(false);
                        *current_request_id_c.borrow_mut() = None;
                        gtk4::prelude::GtkWindowExt::set_focus(&window_c, gtk4::Widget::NONE);
                        window_c.set_visible(false);
                    }
                }
            }
        }
        glib::ControlFlow::Continue
    });

    // Authenticate button — submit password to the current PAM session.
    {
        let shared_c = shared.clone();
        let current_request_id_c = current_request_id.clone();
        let password_entry_c = password_entry.clone();
        let fingerprint_status_c = fingerprint_status.clone();
        auth_button.connect_clicked(move |btn| {
            let Some(request_id) = *current_request_id_c.borrow() else {
                return;
            };
            let password = password_entry_c.text().to_string();
            if shared_c.respond(request_id, &password) {
                password_entry_c.set_sensitive(false);
                btn.set_sensitive(false);
                fingerprint_status_c.set_label("Authenticating...");
            }
        });
    }

    // Enter key on password field triggers auth button.
    {
        let auth_button_c = auth_button.clone();
        password_entry.connect_activate(move |_| {
            if auth_button_c.is_sensitive() {
                auth_button_c.emit_clicked();
            }
        });
    }

    // Cancel button — cancel the current PAM session.
    {
        let shared_c = shared.clone();
        let current_request_id_c = current_request_id.clone();
        let window_c = window.clone();
        cancel_button.connect_clicked(move |_| {
            if let Some(request_id) = *current_request_id_c.borrow() {
                let _ = shared_c.cancel_request(request_id);
                *current_request_id_c.borrow_mut() = None;
            }
            gtk4::prelude::GtkWindowExt::set_focus(&window_c, gtk4::Widget::NONE);
            window_c.set_visible(false);
        });
    }

    // Switching the selected user restarts the session for that identity.
    {
        let shared_c = shared.clone();
        let users_c = users;
        let initializing_c = initializing;
        let current_request_id_c = current_request_id;
        let separator_label_c = separator_label.clone();
        let password_box_c = password_box.clone();
        let password_entry_c = password_entry.clone();
        let auth_button_c = auth_button.clone();
        let fingerprint_status_c = fingerprint_status.clone();
        let fingerprint_label_c = fingerprint_label.clone();
        user_dropdown.connect_selected_notify(move |dropdown| {
            if *initializing_c.borrow() {
                return;
            }

            let Some(request_id) = *current_request_id_c.borrow() else {
                return;
            };
            let selected = dropdown.selected() as usize;
            if selected >= users_c.borrow().len() {
                return;
            }

            if shared_c.select_user(request_id, selected) {
                separator_label_c.set_visible(false);
                password_box_c.set_visible(false);
                password_entry_c.set_text("");
                password_entry_c.set_sensitive(false);
                auth_button_c.set_sensitive(false);
                fingerprint_status_c.set_label("Waiting for authentication...");
                fingerprint_label_c.set_label("🔐");
                fingerprint_status_c.remove_css_class("success");
                fingerprint_status_c.remove_css_class("error");
            }
        });
    }
}
