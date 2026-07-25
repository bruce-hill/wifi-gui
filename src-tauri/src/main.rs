#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use std::process::Command;

// ── Types ─────────────────────────────────────────────────────────────────────
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Network {
    ssid:    String,
    signal:  u8,
    secured: bool,
    in_use:  bool,
    known:   bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WireguardStatus {
    installed: bool,
    up:        bool,
    interface: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InfoState {
    ssid:        String,
    has_profile: bool,
    password:    String,
    autoconnect: Option<bool>,
    bssid:       String,
    channel:     String,
    band:        String,
    rate:        String,
    security:    String,
    ip4_address: String,
    ip4_gateway: String,
    ip4_dns:     String,
}

// ── nmcli helpers ─────────────────────────────────────────────────────────────
fn nmcli(args: &[&str]) -> Result<String, String> {
    let out = Command::new("nmcli")
        .args(args)
        .output()
        .map_err(|e| format!("nmcli: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.is_empty() {
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        } else {
            err
        })
    }
}

// Names of saved Wi-Fi connection profiles (profile name == SSID for
// profiles created by `nmcli device wifi connect`, which is all this app makes).
fn saved_wifi_names() -> std::collections::HashSet<String> {
    let mut names = std::collections::HashSet::new();
    if let Ok(out) = nmcli(&["-t", "-f", "NAME,TYPE", "connection", "show"]) {
        for line in out.lines() {
            let p = split_nmcli(line, 2);
            if p.len() == 2 && p[1] == "802-11-wireless" {
                names.insert(p[0].clone());
            }
        }
    }
    names
}

fn parse_networks(output: &str, saved: &std::collections::HashSet<String>) -> Vec<Network> {
    let mut networks: Vec<Network> = Vec::new();
    for line in output.lines() {
        let parts: Vec<&str> = line.splitn(5, ':').collect();
        if parts.len() < 4 { continue; }
        let in_use  = parts[0].trim() == "*";
        let ssid    = parts[1].replace("\\:", ":").trim().to_string();
        let signal: u8 = parts[2].trim().parse().unwrap_or(0);
        let secured = { let s = parts[3].trim(); !s.is_empty() && s != "--" };
        if ssid.is_empty() || ssid == "--" { continue; }
        if let Some(ex) = networks.iter_mut().find(|n| n.ssid == ssid) {
            if in_use || signal > ex.signal { ex.signal = signal; ex.in_use = in_use; }
            continue;
        }
        let known = saved.contains(&ssid);
        networks.push(Network { ssid, signal, secured, in_use, known });
    }
    networks.sort_by(|a, b| b.in_use.cmp(&a.in_use).then(b.signal.cmp(&a.signal)));
    networks
}

// Split a nmcli -t output line on unescaped ':' into at most max_parts fields.
fn split_nmcli(line: &str, max_parts: usize) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur   = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&':') {
            chars.next();
            cur.push(':');
        } else if c == ':' && parts.len() + 1 < max_parts {
            parts.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    parts.push(cur);
    parts
}

// ── Commands ──────────────────────────────────────────────────────────────────
#[tauri::command]
async fn scan() -> Result<Vec<Network>, String> {
    let saved = saved_wifi_names();
    nmcli(&["-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY",
            "device", "wifi", "list", "--rescan", "yes"])
        .map(|out| parse_networks(&out, &saved))
}

#[tauri::command]
async fn quick_scan() -> Result<Vec<Network>, String> {
    let saved = saved_wifi_names();
    nmcli(&["-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY",
            "device", "wifi", "list", "--rescan", "no"])
        .map(|out| parse_networks(&out, &saved))
}

#[tauri::command]
async fn get_wifi_enabled() -> bool {
    nmcli(&["radio", "wifi"])
        .map(|out| out.trim() == "enabled")
        .unwrap_or(true)
}

#[tauri::command]
async fn set_wifi_enabled(enabled: bool) -> Result<(), String> {
    nmcli(&["radio", "wifi", if enabled { "on" } else { "off" }]).map(|_| ())
}

#[tauri::command]
async fn connect(ssid: String, password: Option<String>) -> Result<(), String> {
    let mut args = vec!["device", "wifi", "connect", ssid.as_str()];
    if let Some(pw) = password.as_deref() {
        args.extend(["password", pw]);
    }
    nmcli(&args).map(|_| ())
}

// Bring up an existing saved profile (no password prompt needed).
#[tauri::command]
async fn connect_saved(ssid: String) -> Result<(), String> {
    nmcli(&["connection", "up", "id", &ssid]).map(|_| ())
}

#[tauri::command]
async fn connect_hidden(ssid: String, password: Option<String>) -> Result<(), String> {
    let mut args = vec!["device", "wifi", "connect", ssid.as_str()];
    if let Some(pw) = password.as_deref() {
        args.extend(["password", pw]);
    }
    args.extend(["hidden", "yes"]);
    nmcli(&args).map(|_| ())
}

#[tauri::command]
async fn set_autoconnect(ssid: String, enabled: bool) -> Result<(), String> {
    nmcli(&["connection", "modify", "id", &ssid, "connection.autoconnect",
            if enabled { "yes" } else { "no" }]).map(|_| ())
}

#[tauri::command]
async fn disconnect(ssid: String) -> Result<(), String> {
    nmcli(&["connection", "down", "id", &ssid]).map(|_| ())
}

#[tauri::command]
async fn forget(ssid: String) -> Result<(), String> {
    nmcli(&["connection", "delete", "id", &ssid]).map(|_| ())
}

#[tauri::command]
async fn save_password(ssid: String, password: String) -> Result<(), String> {
    nmcli(&["connection", "modify", "id", &ssid, "wifi-sec.psk", &password]).map(|_| ())
}

#[tauri::command]
async fn fetch_info(ssid: String, in_use: bool) -> InfoState {
    let mut s = InfoState {
        ssid:        ssid.clone(),
        has_profile: false,
        password:    String::new(),
        autoconnect: None,
        bssid:       String::new(),
        channel:     String::new(),
        band:        String::new(),
        rate:        String::new(),
        security:    String::new(),
        ip4_address: String::new(),
        ip4_gateway: String::new(),
        ip4_dns:     String::new(),
    };

    // 1. Saved profile: autoconnect + PSK
    if let Ok(out) = nmcli(&["--show-secrets", "-t", "-f",
                             "connection.autoconnect,802-11-wireless-security.psk",
                             "connection", "show", "id", &ssid]) {
        s.has_profile = true;
        for line in out.lines() {
            let p = split_nmcli(line, 2);
            if p.len() < 2 { continue; }
            match p[0].as_str() {
                "connection.autoconnect" => {
                    s.autoconnect = match p[1].as_str() {
                        "no" => Some(false),
                        _    => Some(true),   // "yes" or "-1" (default = yes)
                    };
                }
                "802-11-wireless-security.psk" => {
                    let v = p[1].trim().to_string();
                    if !v.is_empty() && v != "--" { s.password = v; }
                }
                _ => {}
            }
        }
    }

    // 2. Scan cache: BSSID, channel, band, rate, security
    if let Ok(out) = nmcli(&["-t", "-f", "SSID,BSSID,CHAN,FREQ,RATE,SECURITY",
                             "device", "wifi", "list"]) {
        for line in out.lines() {
            let p = split_nmcli(line, 6);
            if p.len() < 6 || p[0].trim() != ssid { continue; }
            let bssid = p[1].trim().to_string();
            let chan  = p[2].trim().to_string();
            let freq  = p[3].trim();
            let rate  = p[4].trim();
            let sec   = p[5].trim();
            if bssid != "--" && !bssid.is_empty() { s.bssid = bssid; }
            if chan  != "--" && !chan.is_empty()  { s.channel = chan; }
            let mhz: u32 = freq.split_whitespace().next()
                .and_then(|x| x.parse().ok()).unwrap_or(0);
            s.band = if mhz >= 5925 { "6 GHz".into() }
                     else if mhz >= 4900 { "5 GHz".into() }
                     else if mhz > 0    { "2.4 GHz".into() }
                     else { String::new() };
            if rate != "--" && !rate.is_empty() { s.rate = rate.to_string(); }
            if !sec.is_empty() && sec != "--" {
                s.security = sec.split_whitespace().collect::<Vec<_>>().join("/");
            }
            break;
        }
    }

    // 3. Active IP info (only when connected)
    if in_use {
        if let Ok(out) = nmcli(&["-t", "-f", "IP4.ADDRESS,IP4.GATEWAY,IP4.DNS",
                                 "connection", "show", "--active", "id", &ssid]) {
            for line in out.lines() {
                let p = split_nmcli(line, 2);
                if p.len() < 2 { continue; }
                let field = p[0].as_str();
                let val   = p[1].trim().to_string();
                if val.is_empty() || val == "--" { continue; }
                // Field names may carry an index suffix: IP4.ADDRESS[1]
                if field.starts_with("IP4.ADDRESS") && s.ip4_address.is_empty() {
                    s.ip4_address = val;
                } else if field.starts_with("IP4.GATEWAY") {
                    s.ip4_gateway = val;
                } else if field.starts_with("IP4.DNS") && s.ip4_dns.is_empty() {
                    s.ip4_dns = val;
                }
            }
        }
    }

    s
}

// ── WireGuard ─────────────────────────────────────────────────────────────────
// First WireGuard-type NetworkManager profile, if any: (name, currently active).
// Preferred over wg-quick — NM needs no root and applies DNS itself instead of
// going through resolvconf.
fn nm_wg_profile() -> Option<(String, bool)> {
    let out = nmcli(&["-t", "-f", "NAME,TYPE,DEVICE", "connection", "show"]).ok()?;
    for line in out.lines() {
        let p = split_nmcli(line, 3);
        if p.len() == 3 && p[1] == "wireguard" {
            return Some((p[0].clone(), !p[2].is_empty()));
        }
    }
    None
}

// The interface a fresh `up` should bring up; active interfaces are detected.
fn wg_default_iface() -> String {
    std::env::var("WIFI_GUI_WG_IFACE").unwrap_or_else(|_| "wg0".into())
}

// First active WireGuard interface, via `ip link` (works unprivileged, unlike
// `wg show` or reading /etc/wireguard). Lines look like "9: wg0: <UP,...>".
fn active_wg_iface() -> Option<String> {
    let out = Command::new("ip")
        .args(["-o", "link", "show", "type", "wireguard"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout).lines().find_map(|line| {
        let name = line.split(':').nth(1)?.trim();
        let name = name.split('@').next().unwrap_or(name);
        (!name.is_empty()).then(|| name.to_string())
    })
}

#[tauri::command]
async fn wireguard_status() -> WireguardStatus {
    if let Some((name, active)) = nm_wg_profile() {
        return WireguardStatus { installed: true, up: active, interface: name };
    }
    let installed = Command::new("which")
        .arg("wg-quick")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    match active_wg_iface() {
        Some(iface) => WireguardStatus { installed, up: true, interface: iface },
        None => WireguardStatus { installed, up: false, interface: wg_default_iface() },
    }
}

// wg-quick needs root: try passwordless doas/sudo first (non-interactive, so
// they fail fast if a password would be needed), then fall back to pkexec.
// --disable-internal-agent stops pkexec from prompting on the controlling
// terminal when no graphical polkit agent is around; it errors out instead.
#[tauri::command]
async fn set_wireguard(up: bool, interface: String) -> Result<(), String> {
    let action = if up { "up" } else { "down" };
    // An NM-managed profile needs no privilege escalation at all.
    if let Some((name, _)) = nm_wg_profile() {
        if name == interface {
            return nmcli(&["connection", action, "id", &name]).map(|_| ());
        }
    }
    let mut errs: Vec<String> = Vec::new();
    for runner in [&["doas", "-n"][..], &["sudo", "-n"], &["pkexec", "--disable-internal-agent"]] {
        let out = Command::new(runner[0])
            .args(&runner[1..])
            .args(["wg-quick", action, &interface])
            .output();
        match out {
            Ok(o) if o.status.success() => return Ok(()),
            Ok(o) => {
                let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
                errs.push(if err.is_empty() {
                    format!("{}: {}", runner[0], o.status)
                } else {
                    err
                });
            }
            Err(e) => errs.push(format!("{}: {e}", runner[0])),
        }
    }
    Err(errs.join(" · "))
}

// ── UI scale ──────────────────────────────────────────────────────────────────
// WebKitGTK only honors integer GTK scale factors, unlike winit (used by the
// old egui build), which derives a fractional scale from the monitor's
// physical DPI on X11. Recompute that scale here so the app looks the same.
#[cfg(target_os = "linux")]
fn ui_scale() -> f64 {
    use gdk::prelude::MonitorExt;
    if let Some(s) = std::env::var("WIFI_GUI_SCALE").ok().and_then(|v| v.parse::<f64>().ok()) {
        return s.clamp(0.5, 4.0);
    }
    if let Some(monitor) = gdk::Display::default().and_then(|d| d.monitor(0)) {
        let px = (monitor.geometry().width() * monitor.scale_factor()) as f64;
        let mm = monitor.width_mm() as f64;
        if px > 0.0 && mm > 0.0 {
            let scale = (px * 25.4 / mm) / 96.0;
            if (0.75..=4.0).contains(&scale) {
                return scale;
            }
        }
    }
    1.0
}

// ── main ──────────────────────────────────────────────────────────────────────
fn main() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(target_os = "linux")]
            {
                use tauri::Manager;
                let win = app.get_webview_window("main").expect("main window");
                let scale = ui_scale();
                if (scale - 1.0).abs() > 0.02 {
                    // set_size is ignored on non-resizable GTK windows
                    let _ = win.set_resizable(true);
                    let _ = win.set_size(tauri::LogicalSize::new(360.0 * scale, 500.0 * scale));
                    let _ = win.set_resizable(false);
                    let _ = win.set_zoom(scale);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scan,
            quick_scan,
            get_wifi_enabled,
            set_wifi_enabled,
            connect,
            connect_saved,
            connect_hidden,
            set_autoconnect,
            disconnect,
            forget,
            save_password,
            fetch_info,
            wireguard_status,
            set_wireguard,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
