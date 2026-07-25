# Wifi GUI

This is a simple wrapper around `nmcli` that gives a nice GUI for changing wifi
networks. Importantly, this includes the ability to show wifi passwords, which
is functionality missing from many wifi picker interfaces.

Built with [Tauri](https://tauri.app): the backend (`src-tauri/`) wraps `nmcli`
in Tauri commands, and the frontend (`ui/`) is plain HTML/CSS/JS — no Node or
bundler required.

## Building

Requires `webkit2gtk-4.1` and `gtk3` (already present on most desktop Linux
systems), plus a Rust toolchain.

```sh
cd src-tauri
cargo build --release
```

The binary lands at `src-tauri/target/release/wifi-gui`.
