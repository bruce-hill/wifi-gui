use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Id, Stroke};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;

// ── Palette ───────────────────────────────────────────────────────────────────
// Title bar: deep navy → medium blue gradient
const TITLE_L:    Color32 = Color32::from_rgb(10,  42, 115);
const TITLE_R:    Color32 = Color32::from_rgb(46, 118, 200);
const TITLE_TEXT: Color32 = Color32::WHITE;
const TITLE_H:    f32     = 26.0;
const CLOSE_N:    Color32 = Color32::from_rgb(172, 32, 32);
const CLOSE_H:    Color32 = Color32::from_rgb(208, 50, 50);

// Panel areas (header / footer): warm silver, subtle gradient
const PANEL_T: Color32 = Color32::from_rgb(248, 246, 242);
const PANEL_B: Color32 = Color32::from_rgb(228, 225, 219);

// Content
const WIN_BG:   Color32 = Color32::from_rgb(238, 235, 230);
const LIST_BG:  Color32 = Color32::WHITE;
const SEL_BG:   Color32 = Color32::from_rgb(13, 105, 213);
const SEL_TEXT: Color32 = Color32::WHITE;
const ROW_HOV:  Color32 = Color32::from_rgb(230, 242, 255);
const ROW_CONN: Color32 = Color32::from_rgb(238, 252, 238);
const SEP:      Color32 = Color32::from_rgb(226, 223, 218);

// Text
const TEXT:    Color32 = Color32::from_rgb(22,  22,  22);
const SUBTEXT: Color32 = Color32::from_rgb(112, 109, 104);
const BORDER:  Color32 = Color32::from_rgb(168, 165, 160);
const SUCCESS: Color32 = Color32::from_rgb(28, 132, 28);
const BAR_DIM: Color32 = Color32::from_rgb(178, 175, 170);

// Toggle
const TOGGLE_ON:  Color32 = Color32::from_rgb( 52, 199,  89); // iOS green
const TOGGLE_OFF: Color32 = Color32::from_rgb(142, 142, 147); // neutral gray

// Buttons
const BTN_FACE:   Color32 = Color32::from_rgb(224, 221, 216);
const BTN_CONN:   Color32 = Color32::from_rgb(13,  105, 213); // Connect (blue)
const BTN_DISC:   Color32 = Color32::from_rgb(175,  38,  22); // Disconnect (red)
const BTN_INFO:   Color32 = Color32::from_rgb( 96,  93,  88); // Info (dark gray)
const BTN_FORGET: Color32 = Color32::from_rgb(175,  38,  22);
const BTN_SAVE:   Color32 = Color32::from_rgb(13,  105, 213);
const BTN_DIS_BG: Color32 = Color32::from_rgb(200, 197, 192);
const BTN_DIS_FG: Color32 = Color32::from_rgb(142, 139, 134);

const ROW_H: f32 = 50.0;

// ── Types ─────────────────────────────────────────────────────────────────────
#[derive(Clone, Debug)]
struct Network {
    ssid:    String,
    signal:  u8,
    secured: bool,
    in_use:  bool,
}

struct InfoState {
    ssid:        String,
    has_profile: bool,
    password:    String,
    show_pw:     bool,
    autoconnect: Option<bool>,
    bssid:       String,
    channel:     String,
    band:        String,
    security:    String,
    ip4_address: String,
    ip4_gateway: String,
    ip4_dns:     String,
}

#[derive(Default)]
struct WifiApp {
    networks: Arc<Mutex<Vec<Network>>>,
    scanning: Arc<Mutex<bool>>,
    status:   Arc<Mutex<String>>,

    selected_ssid: Option<String>,

    // Password-entry view (connecting to a secured network for the first time)
    connecting_to: Option<String>,
    password:      String,
    show_password: bool,

    // Info view (view/edit saved connection details)
    info: Option<InfoState>,

    wifi_enabled: bool,
}

// ── nmcli helpers ─────────────────────────────────────────────────────────────
fn parse_networks(output: &str) -> Vec<Network> {
    let mut networks: Vec<Network> = Vec::new();
    for line in output.lines().skip(1) {
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
        networks.push(Network { ssid, signal, secured, in_use });
    }
    networks.sort_by(|a, b| b.in_use.cmp(&a.in_use).then(b.signal.cmp(&a.signal)));
    networks
}

fn scan(networks: Arc<Mutex<Vec<Network>>>, scanning: Arc<Mutex<bool>>, status: Arc<Mutex<String>>) {
    *scanning.lock().unwrap() = true;
    *status.lock().unwrap() = String::new();
    thread::spawn(move || {
        let out = Command::new("nmcli")
            .args(["-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY",
                   "device", "wifi", "list", "--rescan", "yes"])
            .output();
        match out {
            Ok(o) if o.status.success() => {
                *networks.lock().unwrap() = parse_networks(&String::from_utf8_lossy(&o.stdout));
                *status.lock().unwrap() = String::new();
            }
            Ok(o)  => { *status.lock().unwrap() = String::from_utf8_lossy(&o.stderr).trim().to_string(); }
            Err(e) => { *status.lock().unwrap() = format!("nmcli: {e}"); }
        }
        *scanning.lock().unwrap() = false;
    });
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

// Collect all info-panel data synchronously (~150 ms max).
fn fetch_info(ssid: &str, in_use: bool) -> InfoState {
    let mut s = InfoState {
        ssid:        ssid.to_string(),
        has_profile: false,
        password:    String::new(),
        show_pw:     false,
        autoconnect: None,
        bssid:       String::new(),
        channel:     String::new(),
        band:        String::new(),
        security:    String::new(),
        ip4_address: String::new(),
        ip4_gateway: String::new(),
        ip4_dns:     String::new(),
    };

    // 1. Saved profile: autoconnect + PSK
    if let Ok(o) = Command::new("nmcli")
        .args(["--show-secrets", "-t", "-f",
               "connection.autoconnect,802-11-wireless-security.psk",
               "connection", "show", "id", ssid])
        .output()
    {
        if o.status.success() {
            s.has_profile = true;
            for line in String::from_utf8_lossy(&o.stdout).lines() {
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
    }

    // 2. Scan cache: BSSID, channel, band, security
    if let Ok(o) = Command::new("nmcli")
        .args(["-t", "-f", "SSID,BSSID,CHAN,FREQ,SECURITY",
               "device", "wifi", "list"])
        .output()
    {
        if o.status.success() {
            for line in String::from_utf8_lossy(&o.stdout).lines() {
                let p = split_nmcli(line, 5);
                if p.len() < 5 || p[0].trim() != ssid { continue; }
                let bssid = p[1].trim().to_string();
                let chan  = p[2].trim().to_string();
                let freq  = p[3].trim();
                let sec   = p[4].trim();
                if bssid != "--" && !bssid.is_empty() { s.bssid = bssid; }
                if chan   != "--" && !chan.is_empty()  { s.channel = chan; }
                let mhz: u32 = freq.split_whitespace().next()
                    .and_then(|x| x.parse().ok()).unwrap_or(0);
                s.band = if mhz >= 5925 { "6 GHz".into() }
                         else if mhz >= 4900 { "5 GHz".into() }
                         else if mhz > 0    { "2.4 GHz".into() }
                         else { String::new() };
                if !sec.is_empty() && sec != "--" {
                    s.security = sec.split_whitespace().collect::<Vec<_>>().join("/");
                }
                break;
            }
        }
    }

    // 3. Active IP info (only when connected)
    if in_use {
        if let Ok(o) = Command::new("nmcli")
            .args(["-t", "-f", "IP4.ADDRESS,IP4.GATEWAY,IP4.DNS",
                   "connection", "show", "--active", "id", ssid])
            .output()
        {
            if o.status.success() {
                for line in String::from_utf8_lossy(&o.stdout).lines() {
                    // Format: FIELD[n]:value  — split on first unescaped ':'
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
    }

    s
}

fn check_wifi_enabled() -> bool {
    Command::new("nmcli").args(["radio", "wifi"]).output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "enabled")
        .unwrap_or(true)
}

fn run_nmcli_bg(args: Vec<&'static str>, owned: Vec<String>, status: Arc<Mutex<String>>, ok_msg: String) {
    thread::spawn(move || {
        let mut cmd = Command::new("nmcli");
        for a in &args  { cmd.arg(a); }
        for a in &owned { cmd.arg(a); }
        match cmd.output() {
            Ok(o) if o.status.success() => { *status.lock().unwrap() = ok_msg; }
            Ok(o) => {
                let s = String::from_utf8_lossy(&o.stderr).trim().to_string();
                *status.lock().unwrap() = if s.is_empty() {
                    String::from_utf8_lossy(&o.stdout).trim().to_string()
                } else { s };
            }
            Err(e) => { *status.lock().unwrap() = format!("nmcli: {e}"); }
        }
    });
}

// ── Drawing helpers ───────────────────────────────────────────────────────────
fn gradient_h(painter: &egui::Painter, rect: egui::Rect, left: Color32, right: Color32) {
    use egui::epaint::Mesh;
    let mut m = Mesh::with_texture(egui::TextureId::default());
    m.colored_vertex(rect.left_top(),     left);
    m.colored_vertex(rect.right_top(),    right);
    m.colored_vertex(rect.right_bottom(), right);
    m.colored_vertex(rect.left_bottom(),  left);
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(m));
}

fn gradient_v(painter: &egui::Painter, rect: egui::Rect, top: Color32, bot: Color32) {
    use egui::epaint::Mesh;
    let mut m = Mesh::with_texture(egui::TextureId::default());
    m.colored_vertex(rect.left_top(),     top);
    m.colored_vertex(rect.right_top(),    top);
    m.colored_vertex(rect.right_bottom(), bot);
    m.colored_vertex(rect.left_bottom(),  bot);
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(m));
}

fn draw_x(painter: &egui::Painter, center: egui::Pos2, half: f32, color: Color32) {
    let s = Stroke::new(1.6, color);
    painter.line_segment([egui::pos2(center.x - half, center.y - half),
                          egui::pos2(center.x + half, center.y + half)], s);
    painter.line_segment([egui::pos2(center.x + half, center.y - half),
                          egui::pos2(center.x - half, center.y + half)], s);
}

fn draw_signal_bars(painter: &egui::Painter, bl: egui::Pos2, signal: u8, color: Color32) {
    let bar_w = 4.5_f32;
    let gap   = 2.0_f32;
    let max_h = 16.0_f32;
    let active: u8 = match signal { 75..=100 => 4, 50..=74 => 3, 25..=49 => 2, _ => 1 };
    for i in 0..4u8 {
        let h   = max_h * (f32::from(i) + 1.0) / 4.0;
        let x   = bl.x + f32::from(i) * (bar_w + gap);
        let col = if i < active { color } else { BAR_DIM };
        painter.rect_filled(
            egui::Rect::from_min_size(egui::pos2(x, bl.y - h), egui::vec2(bar_w, h)),
            CornerRadius::same(1), col,
        );
    }
}

fn lighten(c: Color32, amt: u8) -> Color32 {
    Color32::from_rgb(c.r().saturating_add(amt), c.g().saturating_add(amt), c.b().saturating_add(amt))
}

fn darken(c: Color32, amt: u8) -> Color32 {
    Color32::from_rgb(c.r().saturating_sub(amt), c.g().saturating_sub(amt), c.b().saturating_sub(amt))
}

// Gradient fill clipped to a rounded rect — avoids rectangular bleed past corners.
fn gradient_v_rounded(painter: &egui::Painter, rect: egui::Rect, top: Color32, bot: Color32, cr: f32) {
    use egui::epaint::Mesh;
    let pi = std::f32::consts::PI;

    let lerp_color = |y: f32| -> Color32 {
        let t = ((y - rect.top()) / rect.height()).clamp(0.0, 1.0);
        Color32::from_rgb(
            (top.r() as f32 + (bot.r() as f32 - top.r() as f32) * t) as u8,
            (top.g() as f32 + (bot.g() as f32 - top.g() as f32) * t) as u8,
            (top.b() as f32 + (bot.b() as f32 - top.b() as f32) * t) as u8,
        )
    };

    // (corner-center, arc-start, arc-end) going clockwise from top-left
    let corners: [(egui::Pos2, f32, f32); 4] = [
        (egui::pos2(rect.left()  + cr, rect.top()    + cr), pi,       pi * 1.5),
        (egui::pos2(rect.right() - cr, rect.top()    + cr), pi * 1.5, pi * 2.0),
        (egui::pos2(rect.right() - cr, rect.bottom() - cr), 0.0,      pi * 0.5),
        (egui::pos2(rect.left()  + cr, rect.bottom() - cr), pi * 0.5, pi),
    ];

    let segs = 5_u32;
    let mut outline: Vec<egui::Pos2> = Vec::new();
    for (center, a0, a1) in &corners {
        for i in 0..=segs {
            let a = a0 + (a1 - a0) * (i as f32 / segs as f32);
            outline.push(egui::pos2(center.x + cr * a.cos(), center.y + cr * a.sin()));
        }
    }

    let center = rect.center();
    let mut m = Mesh::with_texture(egui::TextureId::default());
    let ci = outline.len() as u32;
    for p in &outline { m.colored_vertex(*p, lerp_color(p.y)); }
    m.colored_vertex(center, lerp_color(center.y));
    let n = outline.len() as u32;
    for i in 0..n { m.add_triangle(ci, i, (i + 1) % n); }
    painter.add(egui::Shape::mesh(m));
}

fn draw_toggle(ui: &mut egui::Ui, id: Id, on: bool) -> egui::Response {
    let w = 38.0_f32;
    let h = 22.0_f32;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool(id, on);
        let bg = Color32::from_rgb(
            (TOGGLE_OFF.r() as f32 + (TOGGLE_ON.r() as f32 - TOGGLE_OFF.r() as f32) * t) as u8,
            (TOGGLE_OFF.g() as f32 + (TOGGLE_ON.g() as f32 - TOGGLE_OFF.g() as f32) * t) as u8,
            (TOGGLE_OFF.b() as f32 + (TOGGLE_ON.b() as f32 - TOGGLE_OFF.b() as f32) * t) as u8,
        );
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(11), bg);
        let margin = 2.5_f32;
        let cr = h / 2.0 - margin;
        let cx = rect.left() + h / 2.0 + t * (w - h);
        let cy = rect.center().y;
        // Subtle drop shadow
        p.circle_filled(egui::pos2(cx, cy + 1.0), cr, Color32::from_rgba_unmultiplied(0, 0, 0, 35));
        p.circle_filled(egui::pos2(cx, cy), cr, Color32::WHITE);
    }
    resp
}

fn apply_theme(ctx: &egui::Context) {
    let mut v = egui::Visuals::light();
    v.panel_fill          = WIN_BG;
    v.window_fill         = WIN_BG;
    v.window_stroke       = Stroke::new(1.0, BORDER);
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill   = SEL_BG;
    let cr = CornerRadius::same(3);
    macro_rules! w { ($w:expr, $bg:expr, $str:expr, $fg:expr) => {{
        $w.bg_fill      = $bg;
        $w.bg_stroke    = $str;
        $w.corner_radius = cr;
        $w.fg_stroke    = $fg;
    }}; }
    w!(v.widgets.noninteractive, WIN_BG,
       Stroke::new(1.0, BORDER), Stroke::new(1.0, TEXT));
    w!(v.widgets.inactive, BTN_FACE,
       Stroke::new(1.0, BORDER), Stroke::new(1.0, TEXT));
    w!(v.widgets.hovered,
       Color32::from_rgb(210, 207, 202),
       Stroke::new(1.0, Color32::from_rgb(112, 109, 104)),
       Stroke::new(1.0, TEXT));
    w!(v.widgets.active, BTN_CONN,
       Stroke::NONE, Stroke::new(1.0, Color32::WHITE));
    ctx.set_visuals(v);
}

// ── Panel helpers ─────────────────────────────────────────────────────────────
// Draw a gradient panel background (call first inside a panel show closure).
fn panel_bg(ui: &egui::Ui) {
    gradient_v(ui.painter(), ui.clip_rect(), PANEL_T, PANEL_B);
}

fn colored_btn(
    ui: &mut egui::Ui,
    label: &str,
    fill: Color32,
    text_col: Color32,
    min_w: f32,
    disabled: bool,
) -> egui::Response {
    let border = Color32::from_rgb(
        fill.r().saturating_sub(30),
        fill.g().saturating_sub(30),
        fill.b().saturating_sub(30),
    );
    let mut btn = egui::Button::new(egui::RichText::new(label).size(13.0).color(text_col))
        .fill(fill)
        .stroke(Stroke::new(1.0, border))
        .min_size(egui::vec2(min_w, 26.0));
    if disabled { btn = btn.sense(egui::Sense::empty()); }
    let resp = ui.add(btn);
    gradient_v(ui.painter(), resp.rect,
        Color32::from_rgba_unmultiplied(255, 255, 255, 25),
        Color32::from_rgba_unmultiplied(0, 0, 0, 18));
    if disabled { resp } else { resp.on_hover_cursor(egui::CursorIcon::PointingHand) }
}

fn neutral_btn(ui: &mut egui::Ui, label: &str, min_w: f32) -> egui::Response {
    let resp = ui.add(
        egui::Button::new(egui::RichText::new(label).size(13.0).color(TEXT))
            .fill(BTN_FACE)
            .stroke(Stroke::new(1.0, BORDER))
            .min_size(egui::vec2(min_w, 26.0)),
    );
    gradient_v(ui.painter(), resp.rect,
        Color32::from_rgba_unmultiplied(255, 255, 255, 25),
        Color32::from_rgba_unmultiplied(0, 0, 0, 18));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

// ── App ───────────────────────────────────────────────────────────────────────
impl WifiApp {
    fn do_connect(&mut self, ssid: &str, password: Option<&str>) {
        let ssid_s = ssid.to_string();
        let pw     = password.map(str::to_string);
        let st     = Arc::clone(&self.status);
        *st.lock().unwrap() = format!("Connecting to {ssid_s}…");
        thread::spawn(move || {
            let mut cmd = Command::new("nmcli");
            cmd.args(["device", "wifi", "connect", &ssid_s]);
            if let Some(p) = &pw { cmd.args(["password", p]); }
            match cmd.output() {
                Ok(o) if o.status.success() => { *st.lock().unwrap() = format!("Connected to {ssid_s}"); }
                Ok(o) => {
                    let msg = String::from_utf8_lossy(&o.stderr).trim().to_string();
                    *st.lock().unwrap() = if msg.is_empty() {
                        String::from_utf8_lossy(&o.stdout).trim().to_string()
                    } else { msg };
                }
                Err(e) => { *st.lock().unwrap() = format!("nmcli: {e}"); }
            }
        });
    }

    fn do_disconnect(&self, ssid: &str) {
        let st = Arc::clone(&self.status);
        let s  = ssid.to_string();
        *st.lock().unwrap() = format!("Disconnecting from {s}…");
        run_nmcli_bg(
            vec!["connection", "down", "id"],
            vec![s.clone()],
            st,
            format!("Disconnected from {s}"),
        );
    }
}

impl eframe::App for WifiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        apply_theme(ctx);
        ctx.request_repaint_after(std::time::Duration::from_millis(500));

        let scanning      = *self.scanning.lock().unwrap();
        let status        = self.status.lock().unwrap().clone();
        let networks      = self.networks.lock().unwrap().clone();
        let connected_ssid = networks.iter().find(|n| n.in_use).map(|n| n.ssid.clone());

        // ── Custom title bar ──────────────────────────────────────────────────
        let mut should_close = false;
        egui::TopBottomPanel::top("titlebar")
            .exact_height(TITLE_H)
            .frame(egui::Frame::new())
            .show_separator_line(false)
            .show(ctx, |ui| {
                let full      = ui.max_rect();
                let close_w   = TITLE_H * 1.5;
                let close_rect = egui::Rect::from_min_size(
                    egui::pos2(full.right() - close_w, full.top()),
                    egui::vec2(close_w, TITLE_H),
                );
                let drag_rect = egui::Rect::from_min_max(
                    full.min, egui::pos2(full.right() - close_w, full.bottom()),
                );

                // Gradient title background
                gradient_h(ui.painter(), full, TITLE_L, TITLE_R);
                // Top-to-bottom depth: highlight on top, shadow on bottom
                gradient_v(ui.painter(), full,
                    Color32::from_rgba_unmultiplied(255, 255, 255, 25),
                    Color32::from_rgba_unmultiplied(0, 0, 0, 30));

                // Draggable region
                let drag = ui.interact(drag_rect, Id::new("title_drag"), egui::Sense::drag());
                if drag.dragged() { ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag); }

                // Title text
                ui.painter().text(
                    egui::pos2(full.left() + 10.0, full.center().y),
                    Align2::LEFT_CENTER,
                    "Wi-Fi",
                    FontId::proportional(13.0),
                    TITLE_TEXT,
                );

                // Close button
                let close = ui.interact(close_rect, Id::new("close_btn"), egui::Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                let close_fill = if close.hovered() { CLOSE_H } else { CLOSE_N };
                let close_vis = close_rect.shrink2(egui::vec2(3.0, 3.0));
                gradient_v_rounded(ui.painter(), close_vis,
                    lighten(close_fill, 25), darken(close_fill, 20), 4.0);
                draw_x(ui.painter(), close_rect.center(), 4.5, Color32::WHITE);
                if close.clicked() { should_close = true; }
            });

        if should_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // ── Route to the active view ──────────────────────────────────────────
        if self.info.is_some() {
            self.show_info_view(ctx, &networks);
        } else if let Some(ssid) = self.connecting_to.clone() {
            self.show_password_view(ctx, &ssid, &status);
        } else {
            self.show_network_list(ctx, &networks, &connected_ssid, scanning, &status);
        }
    }
}

// ── Views ─────────────────────────────────────────────────────────────────────
impl WifiApp {
    fn show_network_list(
        &mut self,
        ctx:           &egui::Context,
        networks:      &[Network],
        connected_ssid: &Option<String>,
        scanning:       bool,
        status:         &str,
    ) {
        let mut scan_clicked    = false;
        let mut do_toggle_wifi  = false;
        let wifi_enabled        = self.wifi_enabled;

        // Sub-header
        egui::TopBottomPanel::top("header")
            .frame(egui::Frame::new()
                .inner_margin(egui::Margin { left: 8, right: 8, top: 6, bottom: 6 }))
            .show(ctx, |ui| {
                panel_bg(ui);
                ui.horizontal(|ui| {
                    if wifi_enabled {
                        if let Some(ssid) = connected_ssid {
                            let (dot, _) = ui.allocate_exact_size(
                                egui::vec2(12.0, 14.0), egui::Sense::hover());
                            ui.painter().circle_filled(dot.center(), 4.0, SUCCESS);
                            ui.label(egui::RichText::new(format!("Connected: {ssid}"))
                                .size(12.0).color(TEXT));
                        } else {
                            ui.label(egui::RichText::new("Not connected")
                                .size(12.0).color(SUBTEXT));
                        }
                    } else {
                        ui.label(egui::RichText::new("Wi-Fi Off")
                            .size(12.0).color(SUBTEXT));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if draw_toggle(ui, Id::new("wifi_toggle"), wifi_enabled).clicked() {
                            do_toggle_wifi = true;
                        }
                        if wifi_enabled {
                            ui.add_space(6.0);
                            if scanning {
                                ui.add(egui::Spinner::new().size(14.0).color(SUBTEXT));
                            } else if neutral_btn(ui, "Scan", 52.0).clicked() {
                                scan_clicked = true;
                            }
                        }
                    });
                });
            });

        if do_toggle_wifi {
            self.wifi_enabled = !wifi_enabled;
            if self.wifi_enabled {
                *self.status.lock().unwrap() = "Turning Wi-Fi on…".into();
                let st  = Arc::clone(&self.status);
                let nw  = Arc::clone(&self.networks);
                let sc  = Arc::clone(&self.scanning);
                thread::spawn(move || {
                    let _ = Command::new("nmcli").args(["radio", "wifi", "on"]).output();
                    *st.lock().unwrap() = String::new();
                    scan(nw, sc, st);
                });
            } else {
                *self.networks.lock().unwrap() = Vec::new();
                *self.status.lock().unwrap() = String::new();
                self.selected_ssid = None;
                thread::spawn(|| {
                    let _ = Command::new("nmcli").args(["radio", "wifi", "off"]).output();
                });
            }
        } else if scan_clicked {
            scan(Arc::clone(&self.networks), Arc::clone(&self.scanning), Arc::clone(&self.status));
        }

        // Bottom bar: status | [Info?] [Connect/Disconnect]
        let selected_net = self.selected_ssid.as_ref()
            .and_then(|s| networks.iter().find(|n| n.ssid == *s))
            .cloned();
        let is_in_use = selected_net.as_ref().map(|n| n.in_use).unwrap_or(false);
        let can_act   = selected_net.is_some();

        let mut do_connect    = false;
        let mut do_disconnect = false;
        let mut do_info       = false;

        egui::TopBottomPanel::bottom("bottom_bar")
            .frame(egui::Frame::new()
                .inner_margin(egui::Margin { left: 8, right: 8, top: 7, bottom: 7 }))
            .show(ctx, |ui| {
                panel_bg(ui);
                ui.horizontal(|ui| {
                    // Status text on the left
                    let msg = if !status.is_empty() { status }
                              else if scanning { "Scanning for networks…" }
                              else { "" };
                    ui.label(egui::RichText::new(msg).size(11.0).color(SUBTEXT));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Primary action button
                        if is_in_use {
                            if colored_btn(ui, "Disconnect", BTN_DISC, Color32::WHITE, 95.0, false).clicked() {
                                do_disconnect = true;
                            }
                        } else {
                            let (fill, tc) = if can_act {
                                (BTN_CONN, Color32::WHITE)
                            } else {
                                (BTN_DIS_BG, BTN_DIS_FG)
                            };
                            if colored_btn(ui, "Connect", fill, tc, 85.0, !can_act).clicked() {
                                do_connect = true;
                            }
                        }
                        // Info button (only when a network is selected)
                        if can_act {
                            ui.add_space(4.0);
                            if colored_btn(ui, "Info", BTN_INFO, Color32::WHITE, 52.0, false).clicked() {
                                do_info = true;
                            }
                        }
                    });
                });
            });

        if do_disconnect {
            if let Some(net) = &selected_net {
                self.do_disconnect(&net.ssid);
            }
        } else if do_connect {
            if let Some(net) = selected_net.clone() {
                if net.secured {
                    self.connecting_to = Some(net.ssid.clone());
                    self.password.clear();
                    self.show_password = false;
                } else {
                    self.do_connect(&net.ssid, None);
                }
            }
        } else if do_info {
            if let Some(net) = &selected_net {
                self.info = Some(fetch_info(&net.ssid, net.in_use));
            }
        }

        // Network list
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(LIST_BG).inner_margin(egui::Margin::ZERO))
            .show(ctx, |ui| {
                if !wifi_enabled {
                    ui.centered_and_justified(|ui| {
                        ui.label(egui::RichText::new("Wi-Fi is turned off")
                            .size(13.0).color(SUBTEXT));
                    });
                    return;
                }
                if networks.is_empty() && !scanning {
                    ui.centered_and_justified(|ui| {
                        ui.label(egui::RichText::new("No networks found — click Scan")
                            .size(13.0).color(SUBTEXT));
                    });
                    return;
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for net in networks {
                        let avail_w  = ui.available_width();
                        let selected = self.selected_ssid.as_deref() == Some(&net.ssid);
                        let (rect, resp) = ui.allocate_exact_size(
                            egui::vec2(avail_w, ROW_H), egui::Sense::click());
                        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);

                        if ui.is_rect_visible(rect) {
                            let p = ui.painter();

                            let bg = if selected       { SEL_BG }
                                     else if resp.hovered() { ROW_HOV }
                                     else if net.in_use     { ROW_CONN }
                                     else                   { LIST_BG };
                            p.rect_filled(rect, CornerRadius::ZERO, bg);
                            p.line_segment(
                                [egui::pos2(rect.left(), rect.bottom()),
                                 egui::pos2(rect.right(), rect.bottom())],
                                Stroke::new(1.0, SEP),
                            );

                            let tc  = if selected { SEL_TEXT } else if net.in_use { Color32::from_rgb(24,112,24) } else { TEXT };
                            let sc  = if selected { Color32::from_rgb(185, 210, 248) } else { SUBTEXT };
                            let bc  = if selected { SEL_TEXT } else if net.in_use { Color32::from_rgb(24,112,24) } else { Color32::from_rgb(64,64,64) };

                            draw_signal_bars(p,
                                egui::pos2(rect.left() + 12.0, rect.center().y + 9.0),
                                net.signal, bc);

                            let tx = rect.left() + 52.0;
                            p.text(egui::pos2(tx, rect.center().y - 6.0),
                                Align2::LEFT_CENTER, &net.ssid,
                                FontId::proportional(13.5), tc);

                            let sub = if net.in_use { "Connected" }
                                      else if net.secured { "Secured" }
                                      else { "Open" };
                            p.text(egui::pos2(tx, rect.center().y + 9.0),
                                Align2::LEFT_CENTER, sub,
                                FontId::proportional(11.0), sc);

                            // Right-side icon
                            let rx = rect.right() - 12.0;
                            let cy = rect.center().y;
                            if net.in_use {
                                // Checkmark drawn as two line segments
                                let col = if selected { SEL_TEXT } else { SUCCESS };
                                let o = egui::pos2(rx - 10.0, cy);
                                p.line_segment([egui::pos2(o.x, o.y + 1.0),
                                                egui::pos2(o.x + 4.0, o.y + 5.0)],
                                               Stroke::new(2.0, col));
                                p.line_segment([egui::pos2(o.x + 3.5, o.y + 5.0),
                                                egui::pos2(o.x + 9.0, o.y - 3.0)],
                                               Stroke::new(2.0, col));
                            } else if net.secured {
                                // Lock: simple rect + arc drawn with lines
                                let lx = rx - 7.0;
                                let ly = cy + 1.0;
                                let lw = 8.0_f32;
                                let lh = 6.0_f32;
                                let col = sc;
                                // shackle (top arc approximated by two diagonal lines + top)
                                p.line_segment([egui::pos2(lx + 1.5, ly - 1.0),
                                                egui::pos2(lx + 1.5, ly - 3.5)],
                                               Stroke::new(1.4, col));
                                p.line_segment([egui::pos2(lx + lw - 1.5, ly - 1.0),
                                                egui::pos2(lx + lw - 1.5, ly - 3.5)],
                                               Stroke::new(1.4, col));
                                p.line_segment([egui::pos2(lx + 1.5, ly - 3.5),
                                                egui::pos2(lx + lw - 1.5, ly - 3.5)],
                                               Stroke::new(1.4, col));
                                // body
                                p.rect_filled(
                                    egui::Rect::from_min_size(egui::pos2(lx, ly), egui::vec2(lw, lh)),
                                    CornerRadius::same(1), col);
                            }
                        }

                        if resp.clicked() {
                            self.selected_ssid = Some(net.ssid.clone());
                        }
                    }
                });
            });
    }

    fn show_password_view(&mut self, ctx: &egui::Context, ssid: &str, status: &str) {
        let mut do_connect = false;
        let mut do_cancel  = false;

        egui::TopBottomPanel::bottom("pw_bar")
            .frame(egui::Frame::new()
                .inner_margin(egui::Margin { left: 8, right: 8, top: 7, bottom: 7 }))
            .show(ctx, |ui| {
                panel_bg(ui);
                ui.horizontal(|ui| {
                    if neutral_btn(ui, "Cancel", 75.0).clicked() { do_cancel = true; }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if colored_btn(ui, "Connect", BTN_CONN, Color32::WHITE, 85.0, false).clicked() || enter {
                            do_connect = true;
                        }
                    });
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new()
                .fill(WIN_BG)
                .inner_margin(egui::Margin { left: 20, right: 20, top: 22, bottom: 8 }))
            .show(ctx, |ui| {
                ui.label(egui::RichText::new(
                    format!("Connect to \u{201c}{ssid}\u{201d}"))
                    .size(15.0).strong().color(TEXT));
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Enter the network password:")
                    .size(12.0).color(SUBTEXT));
                ui.add_space(14.0);
                ui.add(egui::TextEdit::singleline(&mut self.password)
                    .password(!self.show_password)
                    .desired_width(f32::INFINITY)
                    .font(FontId::proportional(13.0))
                    .hint_text("Password")).request_focus();
                ui.add_space(8.0);
                ui.checkbox(&mut self.show_password,
                    egui::RichText::new("Show password").size(12.0).color(SUBTEXT));
                if !status.is_empty() {
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new(status).size(11.0).color(SUBTEXT));
                }
            });

        if do_connect {
            let pw = self.password.clone();
            let s  = ssid.to_string();
            self.do_connect(&s, Some(&pw));
            self.connecting_to = None;
            self.password.clear();
        } else if do_cancel {
            self.connecting_to = None;
            self.password.clear();
        }
    }

    fn show_info_view(&mut self, ctx: &egui::Context, networks: &[Network]) {
        // Clone the fields we need for the button bar (avoids holding a borrow into self
        // while the panel closures run).
        let (ssid, has_profile, is_secured, in_use) = {
            let i   = self.info.as_ref().unwrap();
            let net = networks.iter().find(|n| n.ssid == i.ssid);
            (i.ssid.clone(),
             i.has_profile,
             net.map(|n| n.secured).unwrap_or(true),
             net.map(|n| n.in_use).unwrap_or(false))
        };

        let mut go_back   = false;
        let mut do_forget = false;
        let mut do_save   = false;

        egui::TopBottomPanel::bottom("info_bar")
            .frame(egui::Frame::new()
                .inner_margin(egui::Margin { left: 8, right: 8, top: 7, bottom: 7 }))
            .show(ctx, |ui| {
                panel_bg(ui);
                ui.horizontal(|ui| {
                    if neutral_btn(ui, "Back", 60.0).clicked() { go_back = true; }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if has_profile && is_secured {
                            if colored_btn(ui, "Save Password", BTN_SAVE, Color32::WHITE, 108.0, false).clicked() {
                                do_save = true;
                            }
                            ui.add_space(4.0);
                        }
                        if has_profile {
                            if colored_btn(ui, "Forget", BTN_FORGET, Color32::WHITE, 68.0, false).clicked() {
                                do_forget = true;
                            }
                        }
                    });
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new()
                .fill(WIN_BG)
                .inner_margin(egui::Margin { left: 20, right: 20, top: 16, bottom: 8 }))
            .show(ctx, |ui| {
                let info = self.info.as_mut().unwrap();

                ui.label(egui::RichText::new(&info.ssid).size(16.0).strong().color(TEXT));
                ui.add_space(3.0);
                if in_use {
                    ui.horizontal(|ui| {
                        let (dot, _) = ui.allocate_exact_size(
                            egui::vec2(12.0, 14.0), egui::Sense::hover());
                        ui.painter().circle_filled(dot.center(), 4.0, SUCCESS);
                        ui.label(egui::RichText::new("Currently connected")
                            .size(12.0).color(SUCCESS));
                    });
                } else {
                    ui.label(egui::RichText::new("Not connected").size(12.0).color(SUBTEXT));
                }

                ui.add_space(10.0);

                egui::Grid::new("info_grid")
                    .num_columns(2)
                    .spacing([12.0, 5.0])
                    .min_col_width(76.0)
                    .show(ui, |ui| {
                        let lbl = |text: &str| {
                            egui::RichText::new(text).size(12.0).color(SUBTEXT)
                        };
                        let val = |text: &str| {
                            egui::RichText::new(text).size(12.0).color(TEXT)
                        };

                        // ── Network / radio info ──────────────────────────
                        if !info.security.is_empty() {
                            ui.label(lbl("Security"));
                            ui.label(val(&info.security));
                            ui.end_row();
                        }
                        let freq_line = match (info.band.as_str(), info.channel.as_str()) {
                            (b, c) if !b.is_empty() && !c.is_empty() => format!("{b} · Ch. {c}"),
                            (b, _) if !b.is_empty() => b.to_string(),
                            (_, c) if !c.is_empty() => format!("Ch. {c}"),
                            _ => String::new(),
                        };
                        if !freq_line.is_empty() {
                            ui.label(lbl("Frequency"));
                            ui.label(val(&freq_line));
                            ui.end_row();
                        }
                        if !info.bssid.is_empty() && info.bssid != "--" {
                            ui.label(lbl("BSSID"));
                            ui.label(val(&info.bssid));
                            ui.end_row();
                        }

                        // ── IP info (only when connected) ─────────────────
                        if !info.ip4_address.is_empty() {
                            ui.label(lbl("IP Address"));
                            ui.label(val(&info.ip4_address));
                            ui.end_row();
                        }
                        if !info.ip4_gateway.is_empty() {
                            ui.label(lbl("Gateway"));
                            ui.label(val(&info.ip4_gateway));
                            ui.end_row();
                        }
                        if !info.ip4_dns.is_empty() {
                            ui.label(lbl("DNS"));
                            ui.label(val(&info.ip4_dns));
                            ui.end_row();
                        }

                        // ── Profile info ──────────────────────────────────
                        if info.has_profile && is_secured {
                            ui.label(lbl("Password"));
                            ui.add(egui::TextEdit::singleline(&mut info.password)
                                .password(!info.show_pw)
                                .desired_width(f32::INFINITY)
                                .font(FontId::proportional(12.0))
                                .hint_text("(none saved)"));
                            ui.end_row();

                            ui.label("");
                            ui.checkbox(&mut info.show_pw,
                                egui::RichText::new("Show password").size(12.0).color(SUBTEXT));
                            ui.end_row();
                        }

                        if let Some(ac) = info.autoconnect {
                            ui.label(lbl("Auto-connect"));
                            ui.label(val(if ac { "Yes" } else { "No" }));
                            ui.end_row();
                        }
                    });

                if !info.has_profile {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("No saved profile for this network.")
                        .size(12.0).color(SUBTEXT));
                }
            });

        if go_back {
            self.info = None;
        } else if do_forget {
            let st = Arc::clone(&self.status);
            let ok = format!("Forgot {ssid}");
            run_nmcli_bg(vec!["connection", "delete", "id"], vec![ssid.clone()], st, ok);
            if self.selected_ssid.as_deref() == Some(&ssid) { self.selected_ssid = None; }
            self.info = None;
            scan(Arc::clone(&self.networks), Arc::clone(&self.scanning), Arc::clone(&self.status));
        } else if do_save {
            let pw = self.info.as_ref().unwrap().password.clone();
            let st = Arc::clone(&self.status);
            let ok = format!("Password updated for {ssid}");
            run_nmcli_bg(
                vec!["connection", "modify", "id"],
                vec![ssid.clone(), String::from("wifi-sec.psk"), pw],
                st, ok,
            );
            self.info = None;
        }
    }
}

// ── main ──────────────────────────────────────────────────────────────────────
fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Wi-Fi")
            .with_app_id("wifi-gui")
            .with_inner_size([360.0, 500.0])
            .with_min_inner_size([360.0, 500.0])
            .with_max_inner_size([360.0, 500.0])
            .with_decorations(false)
            .with_resizable(false),
        ..Default::default()
    };

    let mut app = WifiApp::default();
    app.wifi_enabled = check_wifi_enabled();
    if app.wifi_enabled {
        scan(Arc::clone(&app.networks), Arc::clone(&app.scanning), Arc::clone(&app.status));
    }
    eframe::run_native("Wi-Fi", options, Box::new(|_cc| Ok(Box::new(app))))
}
