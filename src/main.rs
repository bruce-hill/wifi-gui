use eframe::egui;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Clone, Debug)]
struct Network {
    ssid: String,
    signal: u8, // 0–100
    secured: bool,
    in_use: bool,
}

#[derive(Default)]
struct WifiApp {
    networks: Arc<Mutex<Vec<Network>>>,
    scanning: Arc<Mutex<bool>>,
    status: Arc<Mutex<String>>,
    password: String,
    connecting_to: Option<String>,
    show_password: bool,
}

fn parse_networks(output: &str) -> Vec<Network> {
    let mut networks: Vec<Network> = Vec::new();
    for line in output.lines().skip(1) {
        // nmcli -t -f IN-USE,SSID,SIGNAL,SECURITY with colon separator
        let parts: Vec<&str> = line.splitn(5, ':').collect();
        if parts.len() < 4 {
            continue;
        }
        let in_use = parts[0].trim() == "*";
        let ssid = parts[1].replace("\\:", ":").trim().to_string();
        let signal: u8 = parts[2].trim().parse().unwrap_or(0);
        let security = parts[3].trim();
        let secured = !security.is_empty() && security != "--";

        if ssid.is_empty() || ssid == "--" {
            continue;
        }

        // Deduplicate: keep strongest signal or the active entry.
        if let Some(existing) = networks.iter_mut().find(|n| n.ssid == ssid) {
            if in_use || signal > existing.signal {
                existing.signal = signal;
                existing.in_use = in_use;
            }
            continue;
        }

        networks.push(Network { ssid, signal, secured, in_use });
    }
    networks.sort_by(|a, b| b.in_use.cmp(&a.in_use).then(b.signal.cmp(&a.signal)));
    networks
}

fn scan(networks: Arc<Mutex<Vec<Network>>>, scanning: Arc<Mutex<bool>>, status: Arc<Mutex<String>>) {
    *scanning.lock().unwrap() = true;
    *status.lock().unwrap() = "Scanning…".into();
    thread::spawn(move || {
        let out = Command::new("nmcli")
            .args(["-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "device", "wifi", "list", "--rescan", "yes"])
            .output();
        match out {
            Ok(o) if o.status.success() => {
                let text = String::from_utf8_lossy(&o.stdout);
                *networks.lock().unwrap() = parse_networks(&text);
                *status.lock().unwrap() = String::new();
            }
            Ok(o) => {
                *status.lock().unwrap() = String::from_utf8_lossy(&o.stderr).trim().to_string();
            }
            Err(e) => {
                *status.lock().unwrap() = format!("nmcli error: {e}");
            }
        }
        *scanning.lock().unwrap() = false;
    });
}

fn signal_bars(signal: u8) -> &'static str {
    match signal {
        75..=100 => "▂▄▆█",
        50..=74  => "▂▄▆░",
        25..=49  => "▂▄░░",
        _        => "▂░░░",
    }
}

impl WifiApp {
    fn connect(&mut self, ssid: &str, password: Option<&str>) {
        let ssid = ssid.to_string();
        let password = password.map(str::to_string);
        let status = Arc::clone(&self.status);
        *status.lock().unwrap() = format!("Connecting to {ssid}…");
        thread::spawn(move || {
            let mut cmd = Command::new("nmcli");
            cmd.args(["device", "wifi", "connect", &ssid]);
            if let Some(pw) = &password {
                cmd.args(["password", pw]);
            }
            let out = cmd.output();
            match out {
                Ok(o) if o.status.success() => {
                    *status.lock().unwrap() = format!("Connected to {ssid}");
                }
                Ok(o) => {
                    let msg = String::from_utf8_lossy(&o.stderr).trim().to_string();
                    *status.lock().unwrap() = if msg.is_empty() {
                        String::from_utf8_lossy(&o.stdout).trim().to_string()
                    } else {
                        msg
                    };
                }
                Err(e) => {
                    *status.lock().unwrap() = format!("nmcli error: {e}");
                }
            }
        });
    }
}

impl eframe::App for WifiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll background threads every 500ms.
        ctx.request_repaint_after(std::time::Duration::from_millis(500));

        let scanning = *self.scanning.lock().unwrap();
        let status = self.status.lock().unwrap().clone();
        let networks = self.networks.lock().unwrap().clone();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Wi-Fi");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if scanning {
                        ui.spinner();
                        ui.label("Scanning…");
                    } else if ui.button("⟳  Scan").clicked() {
                        scan(
                            Arc::clone(&self.networks),
                            Arc::clone(&self.scanning),
                            Arc::clone(&self.status),
                        );
                    }
                });
            });

            ui.separator();

            if networks.is_empty() && !scanning {
                ui.centered_and_justified(|ui| ui.label("No networks found. Click Scan."));
            }

            egui::ScrollArea::vertical().show(ui, |ui| {
                for net in &networks {
                    let label = format!(
                        "{} {}  {}{}",
                        signal_bars(net.signal),
                        net.ssid,
                        if net.secured { "🔒" } else { "   " },
                        if net.in_use { " ✓" } else { "" },
                    );
                    let btn = egui::Button::new(
                        egui::RichText::new(&label).monospace(),
                    )
                    .fill(if net.in_use {
                        egui::Color32::from_rgb(30, 70, 30)
                    } else {
                        egui::Color32::TRANSPARENT
                    })
                    .min_size(egui::vec2(ui.available_width(), 36.0));

                    if ui.add(btn).clicked() {
                        if net.secured {
                            self.connecting_to = Some(net.ssid.clone());
                            self.password.clear();
                            self.show_password = false;
                        } else {
                            let ssid = net.ssid.clone();
                            self.connect(&ssid, None);
                        }
                    }
                }
            });

            if !status.is_empty() {
                ui.separator();
                ui.label(&status);
            }
        });

        // Password dialog
        if let Some(ssid) = self.connecting_to.clone() {
            let mut open = true;
            egui::Window::new(format!("Connect to {ssid}"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label("Password:");
                    let pw_resp = ui.add(
                        egui::TextEdit::singleline(&mut self.password)
                            .password(!self.show_password)
                            .desired_width(260.0),
                    );
                    pw_resp.request_focus();
                    ui.checkbox(&mut self.show_password, "Show password");
                    ui.horizontal(|ui| {
                        let connect = ui.button("Connect").clicked();
                        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if connect || enter {
                            let pw = self.password.clone();
                            self.connect(&ssid, Some(&pw));
                            self.connecting_to = None;
                            self.password.clear();
                        }
                        if ui.button("Cancel").clicked() {
                            self.connecting_to = None;
                            self.password.clear();
                        }
                    });
                });
            if !open {
                self.connecting_to = None;
                self.password.clear();
            }
        }
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Wi-Fi")
            .with_inner_size([380.0, 480.0])
            .with_resizable(true),
        ..Default::default()
    };

    let app = WifiApp::default();
    scan(
        Arc::clone(&app.networks),
        Arc::clone(&app.scanning),
        Arc::clone(&app.status),
    );

    eframe::run_native("Wi-Fi", options, Box::new(|_cc| Ok(Box::new(app))))
}
