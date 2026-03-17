# badged

A polkit authentication agent for Linux window managers.

- **Fingerprint support** - works with `pam_fprintd` out of the box. The password field only appears when PAM explicitly requests it.
- **Minimal dependencies** - just GTK4 and polkit. Less code touching your credentials.
- Works on Wayland and X11

## Why do I need a polkit authentication agent?

Polkit handles authorization on Linux. When an app needs elevated privileges, polkit prompts you for your password or fingerprint to verify your identity. GNOME, KDE, and other desktop environments ship their own polkit agents, but if you're running a window manager like sway, i3, or Hyprland, you need to bring your own.

## Requirements

The target system needs:

- **GTK4** - UI toolkit
- **polkit** - provides `libpolkit-agent-1` and `polkit-agent-helper-1`

| Distro | Package |
|--------|---------|
| Fedora | `gtk4 polkit` |
| Debian/Ubuntu | `libgtk-4-1 policykit-1` |
| Arch | `gtk4 polkit` |

## Installation

Download the latest binary from [Releases](https://github.com/jfernandez/badged/releases) and place it in your `$PATH`.

### NixOS / Home Manager

Add badged as a flake input:

```nix
# flake.nix
inputs.badged = {
  url = "github:jfernandez/badged";
  inputs.nixpkgs.follows = "nixpkgs";
};
```

Then import the home-manager module and enable the service:

```nix
# home-manager config
imports = [ inputs.badged.homeManagerModules.default ];
services.badged.enable = true;
```

This creates a systemd user service that starts badged with your Wayland session.

### Building from source

Requires GTK4 and polkit development libraries:

| Distro | Packages |
|--------|----------|
| Fedora | `gtk4-devel polkit-devel` |
| Debian/Ubuntu | `libgtk-4-dev libpolkit-agent-1-dev` |
| Arch | `gtk4 polkit` |

```
cargo install badged
```

Or clone and build:

```
git clone https://github.com/jfernandez/badged
cd badged
cargo install --path .
```

## Usage

Run `badged` when your session starts. It registers with polkit and waits for authentication requests.

For Hyprland, add to `~/.config/hypr/hyprland.conf` (or `autostart.conf` if you split your config):

```
exec-once = badged
```

## How it works

When an application requests elevated privileges, polkit looks for a registered authentication agent. badged uses `libpolkit-agent-1` to register a listener and create PAM sessions. The library spawns `polkit-agent-helper-1` in-process, which handles all PAM interaction — including fingerprint prompts via `pam_fprintd`. badged never runs as root and never handles passwords directly; it passes them to the PAM session which relays them to the helper.
