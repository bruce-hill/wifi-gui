const { invoke } = window.__TAURI__.core;
const appWindow = window.__TAURI__.window.getCurrentWindow();

// ── State ─────────────────────────────────────────────────────────────────────
let networks     = [];
let selectedSsid = null;
let wifiEnabled  = true;
let scanning     = false;
let status       = "";
let info         = null;   // InfoState when the info view is open
let connectingTo = null;   // SSID when the password view is open

const $ = (id) => document.getElementById(id);

// ── Rendering ─────────────────────────────────────────────────────────────────
function showView(name) {
  $("view-list").classList.toggle("hidden", name !== "list");
  $("view-password").classList.toggle("hidden", name !== "password");
  $("view-info").classList.toggle("hidden", name !== "info");
}

function barsFor(signal) {
  const active = signal >= 75 ? 4 : signal >= 50 ? 3 : signal >= 25 ? 2 : 1;
  let html = '<span class="bars">';
  for (let i = 0; i < 4; i++) html += `<i class="${i < active ? "on" : ""}"></i>`;
  return html + "</span>";
}

const CHECK_SVG = `<span class="icon check"><svg width="14" height="12" viewBox="0 0 14 12">
  <path class="stroke" d="M2 6.5 L5.5 10 L12 2" fill="none" stroke-width="2"/></svg></span>`;
const LOCK_SVG = `<span class="icon lock"><svg width="12" height="14" viewBox="0 0 12 14">
  <path class="stroke" d="M3.5 6 V4 a2.5 2.5 0 0 1 5 0 V6" fill="none" stroke-width="1.4"/>
  <rect class="fill" x="2" y="6" width="8" height="6" rx="1"/></svg></span>`;

function renderList() {
  const list = $("network-list");
  if (!wifiEnabled) {
    list.innerHTML = '<div class="empty">Wi-Fi is turned off</div>';
    return;
  }
  if (networks.length === 0 && !scanning) {
    list.innerHTML = '<div class="empty">No networks found — click Scan</div>';
    return;
  }
  list.innerHTML = "";
  for (const net of networks) {
    const row = document.createElement("div");
    row.className = "row";
    if (net.inUse) row.classList.add("in-use");
    if (net.ssid === selectedSsid) row.classList.add("selected");
    const sub = net.inUse ? "Connected" : net.secured ? "Secured" : "Open";
    const icon = net.inUse ? CHECK_SVG : net.secured ? LOCK_SVG : "";
    row.innerHTML = `${barsFor(net.signal)}
      <span class="labels"><div class="ssid"></div><div class="sub">${sub}</div></span>${icon}`;
    row.querySelector(".ssid").textContent = net.ssid;
    row.addEventListener("click", () => { selectedSsid = net.ssid; renderList(); renderChrome(); });
    list.appendChild(row);
  }
}

function renderChrome() {
  // Header
  const conn = networks.find((n) => n.inUse);
  const cs = $("conn-status");
  if (!wifiEnabled) {
    cs.textContent = "Wi-Fi Off";
    cs.classList.remove("connected");
  } else if (conn) {
    cs.textContent = `Connected: ${conn.ssid}`;
    cs.classList.add("connected");
  } else {
    cs.textContent = "Not connected";
    cs.classList.remove("connected");
  }
  $("wifi-toggle").checked = wifiEnabled;
  $("scan-btn").classList.toggle("hidden", !wifiEnabled || scanning);
  $("scan-spinner").classList.toggle("hidden", !wifiEnabled || !scanning);

  // Footer
  $("status-text").textContent =
    status || (scanning ? "Scanning for networks…" : "");
  const sel = networks.find((n) => n.ssid === selectedSsid);
  const action = $("action-btn");
  if (sel && sel.inUse) {
    action.textContent = "Disconnect";
    action.className = "btn disconnect";
    action.disabled = false;
  } else {
    action.textContent = "Connect";
    action.className = "btn connect";
    action.disabled = !sel;
  }
  $("info-btn").classList.toggle("hidden", !sel);
}

function render() {
  renderList();
  renderChrome();
}

// ── Actions ───────────────────────────────────────────────────────────────────
async function refreshQuick() {
  try {
    networks = await invoke("quick_scan");
  } catch (_) { /* keep the old list */ }
  render();
}

async function doScan() {
  scanning = true;
  status = "";
  render();
  try {
    networks = await invoke("scan");
    status = "";
  } catch (e) {
    status = String(e);
  }
  scanning = false;
  render();
}

async function doConnect(ssid, password) {
  status = `Connecting to ${ssid}…`;
  renderChrome();
  try {
    await invoke("connect", { ssid, password: password ?? null });
    status = `Connected to ${ssid}`;
  } catch (e) {
    status = String(e);
  }
  await refreshQuick();
}

async function doDisconnect(ssid) {
  status = `Disconnecting from ${ssid}…`;
  renderChrome();
  try {
    await invoke("disconnect", { ssid });
    status = `Disconnected from ${ssid}`;
  } catch (e) {
    status = String(e);
  }
  await refreshQuick();
}

// ── Password view ─────────────────────────────────────────────────────────────
function openPasswordView(ssid) {
  connectingTo = ssid;
  $("pw-title").textContent = `Connect to “${ssid}”`;
  $("pw-input").value = "";
  $("pw-input").type = "password";
  $("pw-show").checked = false;
  $("pw-status").textContent = "";
  showView("password");
  $("pw-input").focus();
}

function submitPassword() {
  const ssid = connectingTo;
  const pw = $("pw-input").value;
  connectingTo = null;
  showView("list");
  doConnect(ssid, pw);
}

// ── Info view ─────────────────────────────────────────────────────────────────
function infoRow(label, value) {
  const tr = document.createElement("tr");
  const td1 = document.createElement("td");
  const td2 = document.createElement("td");
  td1.textContent = label;
  td2.textContent = value;
  tr.append(td1, td2);
  return tr;
}

async function openInfoView(net) {
  info = await invoke("fetch_info", { ssid: net.ssid, inUse: net.inUse });
  info.secured = net.secured;

  $("info-ssid").textContent = info.ssid;
  const ic = $("info-conn");
  ic.textContent = net.inUse ? "Currently connected" : "Not connected";
  ic.classList.toggle("connected", net.inUse);

  const grid = $("info-grid");
  grid.innerHTML = "";
  if (info.security) grid.append(infoRow("Security", info.security));
  const freq = [info.band, info.channel && `Ch. ${info.channel}`]
    .filter(Boolean).join(" · ");
  if (freq) grid.append(infoRow("Frequency", freq));
  if (info.bssid) grid.append(infoRow("BSSID", info.bssid));
  if (info.ip4Address) grid.append(infoRow("IP Address", info.ip4Address));
  if (info.ip4Gateway) grid.append(infoRow("Gateway", info.ip4Gateway));
  if (info.ip4Dns) grid.append(infoRow("DNS", info.ip4Dns));

  if (info.hasProfile && info.secured) {
    const tr = document.createElement("tr");
    const td1 = document.createElement("td");
    td1.textContent = "Password";
    const td2 = document.createElement("td");
    const input = document.createElement("input");
    input.type = "password";
    input.id = "info-pw";
    input.value = info.password;
    input.placeholder = "(none saved)";
    td2.append(input);
    tr.append(td1, td2);
    grid.append(tr);

    const tr2 = document.createElement("tr");
    const showTd = document.createElement("td");
    const showTd2 = document.createElement("td");
    showTd2.innerHTML =
      '<label class="check"><input type="checkbox" id="info-pw-show" /> Show password</label>';
    tr2.append(showTd, showTd2);
    grid.append(tr2);
    $("info-pw-show").addEventListener("change", (e) => {
      input.type = e.target.checked ? "text" : "password";
    });
  }
  if (info.autoconnect !== null) {
    grid.append(infoRow("Auto-connect", info.autoconnect ? "Yes" : "No"));
  }

  $("info-noprofile").classList.toggle("hidden", info.hasProfile);
  $("info-forget").classList.toggle("hidden", !info.hasProfile);
  $("info-save").classList.toggle("hidden", !(info.hasProfile && info.secured));
  showView("info");
}

// ── Event wiring ──────────────────────────────────────────────────────────────
$("close-btn").addEventListener("click", () => appWindow.close());

$("scan-btn").addEventListener("click", doScan);

$("wifi-toggle").addEventListener("change", async (e) => {
  wifiEnabled = e.target.checked;
  if (wifiEnabled) {
    status = "Turning Wi-Fi on…";
    render();
    try {
      await invoke("set_wifi_enabled", { enabled: true });
      status = "";
      await doScan();
    } catch (err) {
      status = String(err);
      render();
    }
  } else {
    networks = [];
    selectedSsid = null;
    status = "";
    render();
    invoke("set_wifi_enabled", { enabled: false }).catch(() => {});
  }
});

$("action-btn").addEventListener("click", () => {
  const sel = networks.find((n) => n.ssid === selectedSsid);
  if (!sel) return;
  if (sel.inUse) {
    doDisconnect(sel.ssid);
  } else if (sel.secured) {
    openPasswordView(sel.ssid);
  } else {
    doConnect(sel.ssid, null);
  }
});

$("info-btn").addEventListener("click", () => {
  const sel = networks.find((n) => n.ssid === selectedSsid);
  if (sel) openInfoView(sel);
});

$("pw-cancel").addEventListener("click", () => {
  connectingTo = null;
  showView("list");
});
$("pw-connect").addEventListener("click", submitPassword);
$("pw-input").addEventListener("keydown", (e) => {
  if (e.key === "Enter") submitPassword();
});
$("pw-show").addEventListener("change", (e) => {
  $("pw-input").type = e.target.checked ? "text" : "password";
});

$("info-back").addEventListener("click", () => {
  info = null;
  showView("list");
});

$("info-forget").addEventListener("click", async () => {
  const ssid = info.ssid;
  info = null;
  showView("list");
  try {
    await invoke("forget", { ssid });
    status = `Forgot ${ssid}`;
    if (selectedSsid === ssid) selectedSsid = null;
  } catch (e) {
    status = String(e);
  }
  render();
  doScan();
});

$("info-save").addEventListener("click", async () => {
  const ssid = info.ssid;
  const password = $("info-pw").value;
  info = null;
  showView("list");
  try {
    await invoke("save_password", { ssid, password });
    status = `Password updated for ${ssid}`;
  } catch (e) {
    status = String(e);
  }
  render();
});

// ── Init ──────────────────────────────────────────────────────────────────────
(async () => {
  wifiEnabled = await invoke("get_wifi_enabled");
  render();
  if (wifiEnabled) {
    await refreshQuick();
    doScan();
  }
})();
