//! Polkit agent listener — GObject subclass of PolkitAgentListener.
//!
//! Uses glib 0.20 (matching polkit-agent-rs) for GObject subclassing.
//! Communicates with the GTK4 UI via mpsc channels and Arc<SharedState>.

use std::cell::RefCell;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, Weak};

use glib::prelude::*;
use glib::subclass::prelude::*;

use polkit_agent_rs::gio;
use polkit_agent_rs::gio::prelude::*;
use polkit_agent_rs::polkit;
use polkit_agent_rs::subclass::ListenerImpl;
use polkit_agent_rs::traits::ListenerExt;
use polkit_agent_rs::{RegisterFlags, Session};

/// Events sent from the listener to the GTK4 UI.
#[derive(Debug, Clone)]
pub enum UiEvent {
    ShowDialog {
        request_id: u64,
        message: String,
        users: Vec<String>,
    },
    PamInfo(String),
    PamError(String),
    PasswordNeeded,
    AuthComplete { success: bool },
    PolkitCancelled { request_id: u64 },
}

#[derive(Clone)]
struct IdentityChoice {
    user: String,
    identity: polkit::Identity,
}

struct ActiveRequest {
    request_id: u64,
    attempt_id: u64,
    cookie: String,
    selected_user: usize,
    choices: Vec<IdentityChoice>,
    session: Session,
    task: gio::Task<bool>,
}

struct SharedInner {
    next_request_id: u64,
    active: Option<ActiveRequest>,
}

/// State shared between listener and UI for session control.
pub struct SharedState {
    event_tx: mpsc::Sender<UiEvent>,
    inner: Mutex<SharedInner>,
}

impl SharedState {
    pub fn new(event_tx: mpsc::Sender<UiEvent>) -> Arc<Self> {
        Arc::new(Self {
            event_tx,
            inner: Mutex::new(SharedInner {
                next_request_id: 1,
                active: None,
            }),
        })
    }

    pub fn start_request(
        self: &Arc<Self>,
        message: &str,
        cookie: &str,
        identities: Vec<polkit::Identity>,
        task: gio::Task<bool>,
        cancellable: gio::Cancellable,
    ) {
        let choices: Vec<IdentityChoice> = identities
            .into_iter()
            .filter_map(|identity| {
                identity
                    .downcast_ref::<polkit::UnixUser>()
                    .and_then(|user| user.name())
                    .map(|user| IdentityChoice {
                        user: user.to_string(),
                        identity,
                    })
            })
            .collect();

        if choices.is_empty() {
            unsafe {
                task.return_result(Err(glib::Error::new(
                    glib::FileError::Failed,
                    "No valid identities",
                )))
            };
            return;
        }

        let users = choices.iter().map(|choice| choice.user.clone()).collect();
        let session = Session::new(&choices[0].identity, cookie);

        let (request_id, attempt_id, previous) = {
            let mut inner = self.inner.lock().unwrap();
            let request_id = inner.next_request_id;
            inner.next_request_id += 1;

            let active = ActiveRequest {
                request_id,
                attempt_id: 1,
                cookie: cookie.to_owned(),
                selected_user: 0,
                choices,
                session: session.clone(),
                task,
            };
            let previous = inner.active.replace(active);
            (request_id, 1, previous)
        };

        if let Some(previous) = previous {
            self.abort_request(previous, false);
        }

        let _ = self.event_tx.send(UiEvent::ShowDialog {
            request_id,
            message: message.to_owned(),
            users,
        });

        self.attach_session(request_id, attempt_id, &session);

        let tx = self.event_tx.clone();
        let _ = cancellable.connect_cancelled(move |_| {
            let _ = tx.send(UiEvent::PolkitCancelled { request_id });
        });

        session.initiate();
    }

    pub fn respond(&self, request_id: u64, password: &str) -> bool {
        let session = {
            let inner = self.inner.lock().unwrap();
            inner
                .active
                .as_ref()
                .filter(|active| active.request_id == request_id)
                .map(|active| active.session.clone())
        };

        if let Some(session) = session {
            session.response(password);
            true
        } else {
            false
        }
    }

    pub fn cancel_request(&self, request_id: u64) -> bool {
        let active = {
            let mut inner = self.inner.lock().unwrap();
            match inner.active.as_ref() {
                Some(active) if active.request_id == request_id => inner.active.take(),
                _ => None,
            }
        };

        if let Some(active) = active {
            self.abort_request(active, true);
            true
        } else {
            false
        }
    }

    pub fn select_user(self: &Arc<Self>, request_id: u64, user_index: usize) -> bool {
        let (session_to_cancel, session_to_start, attempt_id) = {
            let mut inner = self.inner.lock().unwrap();
            let active = match inner.active.as_mut() {
                Some(active) if active.request_id == request_id => active,
                _ => return false,
            };

            if user_index >= active.choices.len() || user_index == active.selected_user {
                return false;
            }

            active.selected_user = user_index;
            active.attempt_id += 1;

            let next_session = Session::new(&active.choices[user_index].identity, &active.cookie);
            let previous_session = active.session.clone();
            active.session = next_session.clone();

            (previous_session, next_session, active.attempt_id)
        };

        self.attach_session(request_id, attempt_id, &session_to_start);
        session_to_start.initiate();
        session_to_cancel.cancel();
        true
    }

    fn attach_session(self: &Arc<Self>, request_id: u64, attempt_id: u64, session: &Session) {
        let tx = self.event_tx.clone();
        let weak = Arc::downgrade(self);
        session.connect_request(move |_sess, _prompt, _echo_on| {
            if is_active_attempt(&weak, request_id, attempt_id) {
                let _ = tx.send(UiEvent::PasswordNeeded);
            }
        });

        let tx = self.event_tx.clone();
        let weak = Arc::downgrade(self);
        session.connect_show_info(move |_sess, text| {
            if is_active_attempt(&weak, request_id, attempt_id) {
                let _ = tx.send(UiEvent::PamInfo(text.to_owned()));
            }
        });

        let tx = self.event_tx.clone();
        let weak = Arc::downgrade(self);
        session.connect_show_error(move |_sess, text| {
            if is_active_attempt(&weak, request_id, attempt_id) {
                let _ = tx.send(UiEvent::PamError(text.to_owned()));
            }
        });

        let weak = Arc::downgrade(self);
        session.connect_completed(move |_sess, gained_auth| {
            if let Some(shared) = weak.upgrade() {
                shared.finish_from_session(request_id, attempt_id, gained_auth);
            }
        });
    }

    fn finish_from_session(&self, request_id: u64, attempt_id: u64, gained_auth: bool) {
        let active = {
            let mut inner = self.inner.lock().unwrap();
            match inner.active.as_ref() {
                Some(active)
                    if active.request_id == request_id && active.attempt_id == attempt_id =>
                {
                    inner.active.take()
                }
                _ => None,
            }
        };

        if let Some(active) = active {
            if gained_auth {
                unsafe { active.task.return_result(Ok(true)) };
            } else {
                unsafe { active.task.return_result(Err(auth_failed_error())) };
            }
            let _ = self
                .event_tx
                .send(UiEvent::AuthComplete { success: gained_auth });
        }
    }

    fn abort_request(&self, active: ActiveRequest, emit_ui_complete: bool) {
        active.session.cancel();
        unsafe { active.task.return_result(Err(cancelled_error())) };
        if emit_ui_complete {
            let _ = self.event_tx.send(UiEvent::AuthComplete { success: false });
        }
    }
}

fn is_active_attempt(weak: &Weak<SharedState>, request_id: u64, attempt_id: u64) -> bool {
    let Some(shared) = weak.upgrade() else {
        return false;
    };

    let inner = shared.inner.lock().unwrap();
    matches!(
        inner.active.as_ref(),
        Some(active) if active.request_id == request_id && active.attempt_id == attempt_id
    )
}

fn auth_failed_error() -> glib::Error {
    glib::Error::new(glib::FileError::Failed, "Authentication failed")
}

fn cancelled_error() -> glib::Error {
    glib::Error::new(gio::IOErrorEnum::Cancelled, "Authentication cancelled")
}

// --- GObject subclass ---

#[derive(Default)]
pub struct BadgedListenerPriv {
    shared: RefCell<Option<Arc<SharedState>>>,
}

#[glib::object_subclass]
impl ObjectSubclass for BadgedListenerPriv {
    const NAME: &'static str = "BadgedListener";
    type Type = BadgedListener;
    type ParentType = polkit_agent_rs::Listener;
}

impl ObjectImpl for BadgedListenerPriv {}

impl ListenerImpl for BadgedListenerPriv {
    type Message = bool;

    fn initiate_authentication(
        &self,
        _action_id: &str,
        message: &str,
        _icon_name: &str,
        _details: &polkit::Details,
        cookie: &str,
        identities: Vec<polkit::Identity>,
        cancellable: gio::Cancellable,
        task: gio::Task<bool>,
    ) {
        eprintln!("[listener] initiate_authentication");

        if let Some(shared) = self.shared.borrow().clone() {
            shared.start_request(message, cookie, identities, task, cancellable);
        } else {
            unsafe {
                task.return_result(Err(glib::Error::new(
                    glib::FileError::Failed,
                    "Shared state unavailable",
                )))
            };
        }
    }

    fn initiate_authentication_finish(
        &self,
        result: Result<gio::Task<bool>, glib::Error>,
    ) -> bool {
        match result {
            Ok(task) => unsafe { task.propagate() }.unwrap_or(false),
            Err(_) => false,
        }
    }
}

// --- Public GObject wrapper ---

glib::wrapper! {
    pub struct BadgedListener(ObjectSubclass<BadgedListenerPriv>)
        @extends polkit_agent_rs::Listener;
}

impl BadgedListener {
    pub fn new(shared: Arc<SharedState>) -> Self {
        let obj: Self = glib::Object::new();
        *obj.imp().shared.borrow_mut() = Some(shared);
        obj
    }

    /// Register as a polkit agent for the current process's session.
    /// Returns a handle that unregisters on drop — keep it alive for the process lifetime.
    pub fn register_for_current_session(
        &self,
    ) -> Result<impl Drop, glib::Error> {
        let subject = polkit::UnixSession::new_for_process_sync(
            std::process::id() as i32,
            None::<&gio::Cancellable>,
        )
        .expect("Failed to resolve session for current process");

        self.register(
            RegisterFlags::NONE,
            &subject,
            "/org/freedesktop/PolicyKit1/AuthenticationAgent",
            None::<&gio::Cancellable>,
        )
    }
}
