//! Polkit Authentication Agent D-Bus interface implementation.

use anyhow::{bail, Context, Result};
use dbus::arg::{PropMap, RefArg};
use dbus::blocking::Connection;
use dbus::channel::{MatchingReceiver, Sender};
use dbus::message::MatchRule;
use dbus::strings::ErrorName;
use dbus::Message;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

const HELPER_PATH: &str = "/usr/lib/polkit-1/polkit-agent-helper-1";
const AGENT_INTERFACE: &str = "org.freedesktop.PolicyKit1.AuthenticationAgent";

/// Request sent to UI to show auth dialog.
#[derive(Debug, Clone)]
pub struct AuthRequest {
    pub message: String,
    pub users: Vec<String>,
}

/// Signal to cancel the current auth request.
#[derive(Debug, Clone)]
pub struct CancelRequest;

/// Signal to shut down the agent.
#[derive(Debug, Clone)]
pub struct ShutdownRequest;

/// PAM info/error message to display in UI.
#[derive(Debug, Clone)]
pub struct PamMessage {
    pub text: String,
    pub is_error: bool,
}

/// Signal that PAM needs a password.
#[derive(Debug, Clone)]
pub struct PasswordNeeded;

/// Password response from UI.
#[derive(Debug)]
pub struct PasswordResponse {
    pub password: String,
}

/// Signal that authentication is complete.
#[derive(Debug, Clone)]
pub struct AuthComplete {
    pub success: bool,
}

/// User selection changed in UI - restart helper with new user.
#[derive(Debug)]
pub struct UserChange {
    pub username: String,
}

/// User clicked cancel in UI.
#[derive(Debug, Clone)]
pub struct UserCancel;

/// Channels for agent-UI communication.
pub struct AgentChannels {
    pub request_tx: mpsc::Sender<AuthRequest>,
    pub cancel_tx: mpsc::Sender<CancelRequest>,
    pub pam_msg_tx: mpsc::Sender<PamMessage>,
    pub password_needed_tx: mpsc::Sender<PasswordNeeded>,
    pub password_rx: mpsc::Receiver<PasswordResponse>,
    pub auth_complete_tx: mpsc::Sender<AuthComplete>,
    pub user_change_rx: mpsc::Receiver<UserChange>,
    pub user_cancel_rx: mpsc::Receiver<UserCancel>,
    pub shutdown_rx: mpsc::Receiver<ShutdownRequest>,
}

/// Channel references for authentication handling.
struct AuthChannelRefs<'a> {
    request_tx: &'a mpsc::Sender<AuthRequest>,
    pam_msg_tx: &'a mpsc::Sender<PamMessage>,
    password_needed_tx: &'a mpsc::Sender<PasswordNeeded>,
    password_rx: &'a mpsc::Receiver<PasswordResponse>,
    auth_complete_tx: &'a mpsc::Sender<AuthComplete>,
    user_change_rx: &'a mpsc::Receiver<UserChange>,
    user_cancel_rx: &'a mpsc::Receiver<UserCancel>,
}

/// Run the D-Bus agent on the current thread (blocking).
pub fn run_blocking(object_path: &'static str, channels: AgentChannels) -> Result<()> {
    let conn = Connection::new_system().context("Failed to connect to system bus")?;

    // Register our object path
    let rule = MatchRule::new_method_call().with_path(object_path);

    // Register with polkit
    crate::authority::register_agent(&conn, object_path)?;
    eprintln!("Polkit agent registered at {object_path}");

    let AgentChannels {
        request_tx,
        cancel_tx,
        pam_msg_tx,
        password_needed_tx,
        password_rx,
        auth_complete_tx,
        user_change_rx,
        user_cancel_rx,
        shutdown_rx,
    } = channels;

    // Process messages
    conn.start_receive(
        rule,
        Box::new(move |msg: Message, conn: &Connection| {
            let member = msg.member().map(|m| m.to_string());
            let interface = msg.interface().map(|i| i.to_string());

            // Only handle our interface
            if interface.as_deref() != Some(AGENT_INTERFACE) {
                return true;
            }

            match member.as_deref() {
                Some("BeginAuthentication") => {
                    let channels = AuthChannelRefs {
                        request_tx: &request_tx,
                        pam_msg_tx: &pam_msg_tx,
                        password_needed_tx: &password_needed_tx,
                        password_rx: &password_rx,
                        auth_complete_tx: &auth_complete_tx,
                        user_change_rx: &user_change_rx,
                        user_cancel_rx: &user_cancel_rx,
                    };
                    let reply = handle_begin_authentication(&msg, channels);
                    let response = match reply {
                        Ok(()) => msg.method_return(),
                        Err(e) => {
                            eprintln!("Auth error: {e:#}");
                            let err_name: ErrorName = "org.freedesktop.DBus.Error.Failed".into();
                            let err_msg = std::ffi::CString::new(e.to_string()).unwrap();
                            Message::error(&msg, &err_name, &err_msg)
                        }
                    };
                    let _ = conn.send(response);
                }
                Some("CancelAuthentication") => {
                    handle_cancel_authentication(&msg, &cancel_tx);
                    let _ = conn.send(msg.method_return());
                }
                _ => {}
            }

            true
        }),
    );

    loop {
        // Check for shutdown request
        if shutdown_rx.try_recv().is_ok() {
            eprintln!("Shutting down polkit agent...");
            crate::authority::unregister_agent(&conn, object_path)?;
            eprintln!("Polkit agent unregistered");
            return Ok(());
        }

        conn.process(Duration::from_millis(100))?;
    }
}

fn handle_begin_authentication(msg: &Message, channels: AuthChannelRefs<'_>) -> Result<()> {
    // Parse arguments: (action_id, message, icon_name, details, cookie, identities)
    let mut iter = msg.iter_init();

    let _action_id: String = iter.read().context("Failed to read action_id")?;
    let message: String = iter.read().context("Failed to read message")?;
    let _icon_name: String = iter.read().context("Failed to read icon_name")?;
    let _details: PropMap = iter.read().context("Failed to read details")?;
    let cookie: String = iter.read().context("Failed to read cookie")?;
    let identities: Vec<(String, PropMap)> = iter.read().context("Failed to read identities")?;

    // Extract usernames from identities
    let users: Vec<String> = identities
        .into_iter()
        .filter_map(|(kind, details)| {
            if kind != "unix-user" {
                return None;
            }
            let uid = details.get("uid").and_then(|v| v.as_u64())? as u32;
            uid_to_username(uid)
        })
        .collect();

    if users.is_empty() {
        bail!("No valid users in authentication request");
    }

    // Send request to UI to show dialog
    let request = AuthRequest {
        message,
        users: users.clone(),
    };
    channels
        .request_tx
        .send(request)
        .context("Failed to send to UI")?;

    // Start with first user
    let mut current_user = users[0].clone();

    loop {
        // Spawn helper for current user
        let result = run_helper_session(
            &current_user,
            &cookie,
            channels.pam_msg_tx,
            channels.password_needed_tx,
            channels.password_rx,
            channels.user_change_rx,
            channels.user_cancel_rx,
        )?;

        match result {
            HelperResult::Success => {
                let _ = channels
                    .auth_complete_tx
                    .send(AuthComplete { success: true });
                return Ok(());
            }
            HelperResult::Failure => {
                let _ = channels
                    .auth_complete_tx
                    .send(AuthComplete { success: false });
                bail!("Authentication failed");
            }
            HelperResult::UserChanged(new_user) => {
                // User changed selection, restart with new user
                current_user = new_user;
                continue;
            }
            HelperResult::Cancelled => {
                bail!("Authentication cancelled by user");
            }
        }
    }
}

enum HelperResult {
    Success,
    Failure,
    UserChanged(String),
    Cancelled,
}

fn run_helper_session(
    username: &str,
    cookie: &str,
    pam_msg_tx: &mpsc::Sender<PamMessage>,
    password_needed_tx: &mpsc::Sender<PasswordNeeded>,
    password_rx: &mpsc::Receiver<PasswordResponse>,
    user_change_rx: &mpsc::Receiver<UserChange>,
    user_cancel_rx: &mpsc::Receiver<UserCancel>,
) -> Result<HelperResult> {
    eprintln!("[agent] Starting helper for user: {username}");

    let mut child = Command::new(HELPER_PATH)
        .arg(username)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("Failed to spawn polkit-agent-helper-1")?;

    let mut stdin = child.stdin.take().context("Failed to get stdin")?;
    let stdout = child.stdout.take().context("Failed to get stdout")?;

    // Send cookie
    writeln!(stdin, "{cookie}").context("Failed to write cookie")?;

    // Spawn reader thread to avoid blocking on stdout
    let (line_tx, line_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if line_tx.send(line).is_err() {
                break;
            }
        }
    });

    loop {
        // Check for user change
        if let Ok(change) = user_change_rx.try_recv() {
            eprintln!("[agent] User changed to: {}", change.username);
            kill_helper(&mut child);
            return Ok(HelperResult::UserChanged(change.username));
        }

        // Check for user cancel
        if user_cancel_rx.try_recv().is_ok() {
            eprintln!("[agent] User cancelled authentication");
            kill_helper(&mut child);
            return Ok(HelperResult::Cancelled);
        }

        // Check for helper output (non-blocking with timeout)
        let line = match line_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(line)) => line,
            Ok(Err(e)) => {
                kill_helper(&mut child);
                bail!("Failed to read from helper: {e}");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // Reader thread exited (helper closed stdout)
                kill_helper(&mut child);
                return Ok(HelperResult::Failure);
            }
        };

        eprintln!("[helper] {line}");

        match parse_helper_line(&line) {
            HelperMessage::PromptEchoOff(_) => {
                // PAM wants a password - signal UI and wait
                let _ = password_needed_tx.send(PasswordNeeded);

                // Wait for password, but also check for user change and cancel
                let password = loop {
                    if let Ok(change) = user_change_rx.try_recv() {
                        eprintln!("[agent] User changed to: {}", change.username);
                        kill_helper(&mut child);
                        return Ok(HelperResult::UserChanged(change.username));
                    }

                    if user_cancel_rx.try_recv().is_ok() {
                        eprintln!("[agent] User cancelled authentication");
                        kill_helper(&mut child);
                        return Ok(HelperResult::Cancelled);
                    }

                    match password_rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(response) => break response.password,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            kill_helper(&mut child);
                            bail!("UI disconnected");
                        }
                    }
                };

                writeln!(stdin, "{password}").context("Failed to write password")?;
            }
            HelperMessage::TextInfo(text) => {
                let _ = pam_msg_tx.send(PamMessage {
                    text,
                    is_error: false,
                });
            }
            HelperMessage::TextError(text) => {
                let _ = pam_msg_tx.send(PamMessage {
                    text,
                    is_error: true,
                });
            }
            HelperMessage::Success => {
                return Ok(HelperResult::Success);
            }
            HelperMessage::Failure => {
                return Ok(HelperResult::Failure);
            }
            HelperMessage::Unknown(_) => {
                // Ignore unknown messages
            }
        }
    }
}

fn kill_helper(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn handle_cancel_authentication(_msg: &Message, cancel_tx: &mpsc::Sender<CancelRequest>) {
    let _ = cancel_tx.send(CancelRequest);
}

fn uid_to_username(uid: u32) -> Option<String> {
    let passwd = std::fs::read_to_string("/etc/passwd").ok()?;
    parse_username_from_passwd(&passwd, uid)
}

fn parse_username_from_passwd(passwd_content: &str, uid: u32) -> Option<String> {
    for line in passwd_content.lines() {
        let fields: Vec<&str> = line.split(':').collect();
        if fields.len() >= 3 && !fields[0].is_empty() {
            if let Ok(entry_uid) = fields[2].parse::<u32>() {
                if entry_uid == uid {
                    return Some(fields[0].to_string());
                }
            }
        }
    }
    None
}

/// Parsed message from polkit-agent-helper-1
#[derive(Debug, PartialEq)]
enum HelperMessage {
    PromptEchoOff(String),
    TextInfo(String),
    TextError(String),
    Success,
    Failure,
    Unknown(String),
}

fn parse_helper_line(line: &str) -> HelperMessage {
    if let Some(prompt) = line.strip_prefix("PAM_PROMPT_ECHO_OFF") {
        HelperMessage::PromptEchoOff(prompt.trim().to_string())
    } else if let Some(info) = line.strip_prefix("PAM_TEXT_INFO") {
        HelperMessage::TextInfo(info.trim().to_string())
    } else if let Some(error) = line.strip_prefix("PAM_TEXT_ERROR") {
        HelperMessage::TextError(error.trim().to_string())
    } else if line == "SUCCESS" {
        HelperMessage::Success
    } else if line == "FAILURE" {
        HelperMessage::Failure
    } else {
        HelperMessage::Unknown(line.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_username_from_passwd() {
        let passwd = "\
root:x:0:0:root:/root:/bin/bash
bin:x:1:1:bin:/bin:/sbin/nologin
nobody:x:65534:65534:Kernel Overflow User:/:/sbin/nologin
jose:x:1000:1000:Jose:/home/jose:/bin/bash";

        assert_eq!(parse_username_from_passwd(passwd, 0), Some("root".into()));
        assert_eq!(parse_username_from_passwd(passwd, 1), Some("bin".into()));
        assert_eq!(
            parse_username_from_passwd(passwd, 65534),
            Some("nobody".into())
        );
        assert_eq!(
            parse_username_from_passwd(passwd, 1000),
            Some("jose".into())
        );
        assert_eq!(parse_username_from_passwd(passwd, 9999), None);
    }

    #[test]
    fn test_parse_username_malformed_lines() {
        let passwd = "\
root:x:0:0:root:/root:/bin/bash
malformed line
:x:2:2::/:/sbin/nologin
short:x";

        assert_eq!(parse_username_from_passwd(passwd, 0), Some("root".into()));
        assert_eq!(parse_username_from_passwd(passwd, 2), None);
    }

    #[test]
    fn test_parse_helper_line_success() {
        assert_eq!(parse_helper_line("SUCCESS"), HelperMessage::Success);
    }

    #[test]
    fn test_parse_helper_line_failure() {
        assert_eq!(parse_helper_line("FAILURE"), HelperMessage::Failure);
    }

    #[test]
    fn test_parse_helper_line_prompt() {
        assert_eq!(
            parse_helper_line("PAM_PROMPT_ECHO_OFF Password:"),
            HelperMessage::PromptEchoOff("Password:".into())
        );
        assert_eq!(
            parse_helper_line("PAM_PROMPT_ECHO_OFF"),
            HelperMessage::PromptEchoOff("".into())
        );
    }

    #[test]
    fn test_parse_helper_line_text_info() {
        assert_eq!(
            parse_helper_line("PAM_TEXT_INFO Place your finger on the reader"),
            HelperMessage::TextInfo("Place your finger on the reader".into())
        );
    }

    #[test]
    fn test_parse_helper_line_text_error() {
        assert_eq!(
            parse_helper_line("PAM_TEXT_ERROR Authentication failed"),
            HelperMessage::TextError("Authentication failed".into())
        );
    }

    #[test]
    fn test_parse_helper_line_unknown() {
        assert_eq!(
            parse_helper_line("SOMETHING_ELSE"),
            HelperMessage::Unknown("SOMETHING_ELSE".into())
        );
    }
}
