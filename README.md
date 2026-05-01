# stutter

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

### From GitHub Releases

Download the latest pre-built binary from the [releases page](https://github.com/Kleshzz/stutter/releases), make it executable, and move it to your path:

```bash
chmod +x stutter
mv stutter ~/.local/bin/
```

### From Source

```bash
cargo build --release
cp target/release/stutter ~/.local/bin/
```

Running as a systemd user service:

```bash
cp stutter.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now stutter
```

## Hyprland autostart

Alternatively, add to your `hyprland.conf`:

```
exec-once = stutter
```

## License

MIT