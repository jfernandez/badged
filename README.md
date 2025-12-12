# badged

A polkit authentication agent for Linux window managers.

- **Fingerprint support** - works with `pam_fprintd` out of the box. The password field only appears when PAM explicitly requests it.
- **Minimal dependencies** - just GTK and D-Bus. Less code touching your credentials.

## Why do I need a polkit authentication agent?

Polkit handles authorization on Linux. When an app needs elevated privileges, polkit prompts you for your password or fingerprint to verify your identity. GNOME, KDE, and other desktop environments ship their own polkit agents, but if you're running a window manager like sway, i3, or Hyprland, you need to bring your own.

## Installation

Requires GTK4 and D-Bus development libraries:

- Fedora: `gtk4-devel dbus-devel`
- Debian/Ubuntu: `libgtk-4-dev libdbus-1-dev`

```
cargo install badged
```

Or build from source:

```
git clone https://github.com/jfernandez/badged
cd badged
cargo install --path .
```

## Usage

Run `badged` when your session starts. It registers with polkit over D-Bus and waits for authentication requests.

For Hyprland, add to `~/.config/hypr/hyprland.conf` (or `autostart.conf` if you split your config):

```
exec-once = badged
```

## How it works

When an application requests elevated privileges, polkit looks for a registered authentication agent and calls its `BeginAuthentication` method over D-Bus. badged shows a dialog, spawns the standard `polkit-agent-helper-1` binary (provided by your distro), and relays credentials through it. The helper handles all PAM interaction, so badged never runs as root and never handles passwords directly. It just pipes them to the helper's stdin.