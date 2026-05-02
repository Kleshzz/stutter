# stutter

[![Crates.io](https://img.shields.io/crates/v/stutter-daemon.svg)](https://crates.io/crates/stutter-daemon)
[![CI](https://github.com/Kleshzz/stutter/actions/workflows/ci.yml/badge.svg)](https://github.com/Kleshzz/stutter/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

A focus-aware process priority daemon for Hyprland.

When you switch windows, stutter automatically raises the CPU priority of the
focused process and restores the previous one. This reduces input latency and
makes the active window feel more responsive under load.

## How it works

stutter connects to Hyprland's event socket and listens for `activewindow`
events. On each focus change it calls `setpriority(2)` to set the focused
process to nice `-5` and resets the previously focused process back to `0`.
On exit, the last focused process is restored to the default priority.

## Requirements

- Hyprland
- Permission to lower nice values - either run as root, or grant the binary
  `CAP_SYS_NICE`:

```
sudo setcap cap_sys_nice+ep ~/.local/bin/stutter
```

## Installation

### AUR (recommended)
```bash
yay -S stutter-daemon
# or
paru -S stutter-daemon
```

### Pre-built binary
```bash
curl -L https://github.com/Kleshzz/stutter/releases/latest/download/stutter \
  -o ~/.local/bin/stutter
chmod +x ~/.local/bin/stutter
```

### From source
```bash
cargo install stutter-daemon
# or build manually:
cargo build --release
cp target/release/stutter ~/.local/bin/
```

## Setup

### Grant permissions (pick one)
```bash
# Option A - capability (recommended)
sudo setcap cap_sys_nice+ep ~/.local/bin/stutter

# Option B - run as root (not recommended)
```

### Autostart

**systemd (recommended):**
```bash
cp stutter.service ~/.config/systemd/user/
systemctl --user enable --now stutter
```

**hyprland.conf:**
```
exec-once = stutter
```

## License

MIT