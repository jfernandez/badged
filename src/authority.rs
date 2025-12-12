//! Polkit Authority D-Bus proxy for agent registration.

use anyhow::{Context, Result};
use dbus::arg::{PropMap, RefArg, Variant};
use dbus::blocking::Connection;
use std::time::Duration;

const POLKIT_SERVICE: &str = "org.freedesktop.PolicyKit1";
const POLKIT_PATH: &str = "/org/freedesktop/PolicyKit1/Authority";
const POLKIT_INTERFACE: &str = "org.freedesktop.PolicyKit1.Authority";

/// Register as a polkit authentication agent.
pub fn register_agent(conn: &Connection, object_path: &str) -> Result<()> {
    let proxy = conn.with_proxy(POLKIT_SERVICE, POLKIT_PATH, Duration::from_secs(10));

    let subject = build_subject()?;
    let locale = std::env::var("LANG").unwrap_or_else(|_| "en_US.UTF-8".to_string());

    proxy
        .method_call::<(), _, _, _>(
            POLKIT_INTERFACE,
            "RegisterAuthenticationAgent",
            (subject, &locale, object_path),
        )
        .context("Failed to register authentication agent")?;

    Ok(())
}

/// Unregister as a polkit authentication agent.
pub fn unregister_agent(conn: &Connection, object_path: &str) -> Result<()> {
    let proxy = conn.with_proxy(POLKIT_SERVICE, POLKIT_PATH, Duration::from_secs(10));

    let subject = build_subject()?;

    proxy
        .method_call::<(), _, _, _>(
            POLKIT_INTERFACE,
            "UnregisterAuthenticationAgent",
            (subject, object_path),
        )
        .context("Failed to unregister authentication agent")?;

    Ok(())
}

/// Build a polkit Subject for the current session.
/// Format: (kind: String, details: Dict<String, Variant>)
fn build_subject() -> Result<(&'static str, PropMap)> {
    let session_id = std::env::var("XDG_SESSION_ID").context("XDG_SESSION_ID not set")?;

    let mut details: PropMap = PropMap::new();
    details.insert(
        "session-id".to_string(),
        Variant(Box::new(session_id) as Box<dyn RefArg>),
    );

    Ok(("unix-session", details))
}
