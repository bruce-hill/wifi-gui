const { invoke } = window.__TAURI__.core;
const appWindow = window.__TAURI__.window.getCurrentWindow();

// ── State ─────────────────────────────────────────────────────────────────────
let networks     = [];
let selectedSsid = null;
let wifiEnabled  = true;
let scanning     = false;
let busy         = false;  // a connect/disconnect is in flight
let status       = "";
let statusError  = false;
let statusTimer  = null;
let info         = null;   // InfoState when the info view is open
let infoNet      = null;   // the Network the info view was opened for
let connectingTo = null;   // SSID when the password view is open
let hiddenJoin   = false;  // password view is in join-hidden-network mode
let forgetArmed  = false;
let forgetTimer  = null;

const $ = (id) => document.getElementById(id);

// ── Rendering ─────────────────────────────────────────────────────────────────
function currentView() {
  for (const name of ["list", "password", "info"]) {
    if (!$("view-" + name).classList.contains("hidden")) return name;
  }
  return "list";
}

function showView(name) {
  $("view-list").classList.toggle("hidden", name !== "list");
  $("view-password").classList.toggle("hidden", name !== "password");
  $("view-info").classList.toggle("hidden", name !== "info");
}

function setStatus(msg, { error = false, sticky = false } = {}) {
  status = msg;
  statusError = error;
  clearTimeout(statusTimer);
  if (msg && !sticky) {
    statusTimer = setTimeout(() => { status = ""; statusError = false; renderChrome(); }, 6000);
  }
  renderChrome();
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

const WIFI_EMPTY_SVG = `<svg width="38" height="31" viewBox="0 0 24 20" fill="none"
    stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
  <path d="M2.4 6.8 a14 14 0 0 1 19.2 0"/>
  <path d="M5.6 10.6 a9.3 9.3 0 0 1 12.8 0"/>
  <path d="M8.8 14.3 a4.8 4.8 0 0 1 6.4 0"/>
  <circle cx="12" cy="17.4" r="1.5" fill="currentColor" stroke="none"/></svg>`;
const CHEV_SVG = `<span class="icon chev"><svg width="7" height="12" viewBox="0 0 7 12">
  <path class="stroke" d="M1 1 L6 6 L1 11" fill="none" stroke-width="1.6"
        stroke-linecap="round" stroke-linejoin="round"/></svg></span>`;

function renderList() {
  const list = $("network-list");
  if (!wifiEnabled) {
    list.innerHTML = `<div class="empty">${WIFI_EMPTY_SVG}<div>Wi-Fi is turned off</div></div>`;
    return;
  }
  if (networks.length === 0) {
    list.innerHTML = `<div class="empty">${WIFI_EMPTY_SVG}<div>${
      scanning ? "Scanning for networks…" : "No networks found"}</div></div>`;
    return;
  }
  list.innerHTML = "";
  const card = el("div", "info-card list-card");
  for (const net of networks) {
    const row = el("div", "row");
    if (net.inUse) row.classList.add("in-use");
    if (net.ssid === selectedSsid) row.classList.add("selected");
    const sub = net.inUse ? "Connected"
              : net.known ? "Saved"
              : net.secured ? "Secured" : "Open";
    const icon = net.inUse ? CHECK_SVG : net.secured ? LOCK_SVG : "";
    row.title = `Signal: ${net.signal}%`;
    row.innerHTML = `${barsFor(net.signal)}
      <span class="labels"><div class="ssid"></div><div class="sub">${sub}</div></span>${icon}${CHEV_SVG}`;
    row.querySelector(".ssid").textContent = net.ssid;
    row.addEventListener("click", () => {
      if (busy) return;
      selectedSsid = net.ssid;
      openInfoView(net);
    });
    card.appendChild(row);
  }
  list.appendChild(card);

  const otherCard = el("div", "info-card other-card");
  const other = el("div", "row other");
  other.innerHTML = `<span class="labels"><div class="ssid">Other network…</div>
    <div class="sub">Join a hidden network by name</div></span>`;
  other.addEventListener("click", () => { if (!busy) openPasswordView(null, true); });
  otherCard.appendChild(other);
  list.appendChild(otherCard);
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
  const scanBtn = $("scan-btn");
  scanBtn.classList.toggle("hidden", !wifiEnabled);
  scanBtn.classList.toggle("spinning", scanning);
  scanBtn.disabled = scanning || busy;
  scanBtn.title = scanning ? "Scanning…" : "Scan for networks";

  // Footer
  const st = $("status-text");
  st.textContent = status || (scanning ? "Scanning for networks…" : "");
  st.classList.toggle("error", statusError && !!status);
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
  setStatus("");
  render();
  try {
    networks = await invoke("scan");
  } catch (e) {
    setStatus(String(e), { error: true, sticky: true });
  }
  scanning = false;
  render();
}

// ── Password view ─────────────────────────────────────────────────────────────
function openPasswordView(ssid, hidden) {
  connectingTo = ssid;
  hiddenJoin = hidden;
  $("pw-title").textContent = hidden ? "Join hidden network" : `Connect to “${ssid}”`;
  $("pw-sub").textContent = hidden
    ? "Enter the network name and password (leave the password blank for open networks):"
    : "Enter the network password:";
  $("pw-ssid").classList.toggle("hidden", !hidden);
  $("pw-ssid").value = "";
  $("pw-input").value = "";
  $("pw-input").type = "password";
  $("pw-show").checked = false;
  $("pw-status").textContent = "";
  $("pw-status").classList.remove("error");
  setPwBusy(false);
  showView("password");
  (hidden ? $("pw-ssid") : $("pw-input")).focus();
}

function setPwBusy(b, msg = "") {
  busy = b;
  $("pw-spinner").classList.toggle("hidden", !b);
  $("pw-cancel").disabled = b;
  $("pw-ssid").disabled = b;
  $("pw-input").disabled = b;
  $("pw-status").textContent = msg;
  $("pw-status").classList.remove("error");
  updatePwButton();
}

function updatePwButton() {
  const ssid = hiddenJoin ? $("pw-ssid").value.trim() : connectingTo;
  const pw = $("pw-input").value;
  // WPA passphrases are 8–63 chars; hidden networks may be open, so allow blank there.
  const pwOk = hiddenJoin ? (pw.length === 0 || pw.length >= 8) : pw.length >= 8;
  $("pw-connect").disabled = busy || !ssid || !pwOk;
}

async function submitPassword() {
  if (busy || $("pw-connect").disabled) return;
  const ssid = hiddenJoin ? $("pw-ssid").value.trim() : connectingTo;
  const pw = $("pw-input").value;
  setPwBusy(true, `Connecting to ${ssid}…`);
  try {
    if (hiddenJoin) {
      await invoke("connect_hidden", { ssid, password: pw || null });
    } else {
      await invoke("connect", { ssid, password: pw });
    }
    busy = false;
    connectingTo = null;
    showView("list");
    setStatus(`Connected to ${ssid}`);
    refreshQuick();
  } catch (e) {
    // A failed attempt leaves behind a broken profile with the bad password;
    // clean it up so the network doesn't show as "Saved". (Safe: this view is
    // only reachable for networks with no pre-existing profile.)
    if (!hiddenJoin) invoke("forget", { ssid }).catch(() => {});
    setPwBusy(false);
    $("pw-status").textContent = String(e);
    $("pw-status").classList.add("error");
    $("pw-input").focus();
    $("pw-input").select();
  }
}

// ── Info view ─────────────────────────────────────────────────────────────────
async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch (_) {
    const ta = document.createElement("textarea");
    ta.value = text;
    document.body.append(ta);
    ta.select();
    const ok = document.execCommand("copy");
    ta.remove();
    return ok;
  }
}

const COPY_SVG = `<svg width="13" height="13" viewBox="0 0 13 13">
  <rect x="4.2" y="4.2" width="7" height="7" rx="1.4" fill="none" stroke="currentColor" stroke-width="1.3"/>
  <path d="M8.8 1.8 H3.2 A1.4 1.4 0 0 0 1.8 3.2 V8.8" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/></svg>`;
const TICK_SVG = `<svg width="13" height="13" viewBox="0 0 13 13">
  <path d="M2.5 7 L5.3 9.8 L10.5 3.5" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
const EYE_SVG = `<svg width="14" height="13" viewBox="0 0 14 13">
  <path d="M1.5 6.5 C3.5 3.2 10.5 3.2 12.5 6.5 C10.5 9.8 3.5 9.8 1.5 6.5 Z" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round"/>
  <circle cx="7" cy="6.5" r="1.7" fill="currentColor"/></svg>`;
const EYE_OFF_SVG = `<svg width="14" height="13" viewBox="0 0 14 13">
  <path d="M1.5 6.5 C3.5 3.2 10.5 3.2 12.5 6.5 C10.5 9.8 3.5 9.8 1.5 6.5 Z" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round"/>
  <circle cx="7" cy="6.5" r="1.7" fill="currentColor"/>
  <path d="M3 11.5 L11 1.5" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/></svg>`;

function el(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text != null) e.textContent = text;
  return e;
}

function iconBtn(svg, title) {
  const b = el("button", "icon-btn");
  b.title = title;
  b.innerHTML = svg;
  return b;
}

function copyIconBtn(getValue, title = "Copy") {
  const b = iconBtn(COPY_SVG, title);
  b.addEventListener("click", async () => {
    if (await copyText(getValue())) {
      b.innerHTML = TICK_SVG;
      b.classList.add("done");
      setTimeout(() => { b.innerHTML = COPY_SVG; b.classList.remove("done"); }, 1200);
    }
  });
  return b;
}

function infoCard(parent, heading) {
  if (heading) parent.append(el("div", "info-sec", heading));
  const card = el("div", "info-card");
  parent.append(card);
  return card;
}

function addRow(card, label, valueNode, copyGetter) {
  const row = el("div", "irow");
  row.append(el("span", "ilabel", label), valueNode);
  if (copyGetter) row.append(copyIconBtn(copyGetter));
  card.append(row);
  return row;
}

const val = (text) => el("span", "ival", text);

async function openInfoView(net) {
  info = await invoke("fetch_info", { ssid: net.ssid, inUse: net.inUse });
  info.secured = net.secured;
  infoNet = net;

  $("info-ssid").textContent = info.ssid;
  const ic = $("info-conn");
  ic.textContent = net.inUse ? "Currently connected" : "Not connected";
  ic.classList.toggle("connected", net.inUse);
  $("info-status").textContent = "";
  $("info-status").classList.remove("error");

  const act = $("info-action");
  act.textContent = net.inUse ? "Disconnect" : "Connect";
  act.className = net.inUse ? "btn disconnect" : "btn connect";
  act.disabled = false;

  const body = $("info-body");
  body.innerHTML = "";

  // ── Saved profile card (password first — it's the main event) ──
  if (info.hasProfile) {
    const ssid = info.ssid;
    const prof = infoCard(body, null);

    if (info.secured) {
      const savedPw = info.password;
      const input = document.createElement("input");
      input.type = "password";
      input.id = "info-pw";
      input.className = "pw";
      input.value = savedPw;
      input.placeholder = "(none saved)";
      input.autocomplete = "off";
      input.title = "Click to edit";
      input.addEventListener("input", () => {
        $("info-save").classList.toggle("hidden", input.value === savedPw);
      });

      const eye = iconBtn(EYE_SVG, "Show password");
      eye.addEventListener("click", () => {
        const show = input.type === "password";
        input.type = show ? "text" : "password";
        eye.innerHTML = show ? EYE_OFF_SVG : EYE_SVG;
        eye.title = show ? "Hide password" : "Show password";
      });

      const row = el("div", "irow");
      row.append(el("span", "ilabel", "Password"), input, eye,
                 copyIconBtn(() => input.value, "Copy password"));
      prof.append(row);
    }

    const acRow = el("div", "irow");
    acRow.append(el("span", "ilabel", "Auto-connect"), el("span", "ival"));
    const toggle = el("label", "toggle small");
    const cb = document.createElement("input");
    cb.type = "checkbox";
    cb.checked = info.autoconnect !== false;
    toggle.append(cb, el("span", "knob"));
    acRow.append(toggle);
    prof.append(acRow);
    cb.addEventListener("change", async (e) => {
      try {
        await invoke("set_autoconnect", { ssid, enabled: e.target.checked });
        $("info-status").textContent = "";
      } catch (err) {
        e.target.checked = !e.target.checked;
        $("info-status").textContent = String(err);
      }
    });
  }

  // ── Details card ──
  const details = infoCard(body, info.hasProfile ? "Details" : null);
  const sig = el("span", "ival sigval");
  sig.innerHTML = `${barsFor(net.signal)}<span>${net.signal}%</span>`;
  addRow(details, "Signal", sig);
  if (info.security) addRow(details, "Security", val(info.security));
  const freq = [info.band, info.channel && `Ch. ${info.channel}`]
    .filter(Boolean).join(" · ");
  if (freq) addRow(details, "Frequency", val(freq));
  if (info.rate) addRow(details, "Max rate", val(info.rate));
  if (info.bssid) addRow(details, "BSSID", val(info.bssid), () => info.bssid);

  // ── Network card (populated only while connected) ──
  if (info.ip4Address || info.ip4Gateway || info.ip4Dns) {
    const netc = infoCard(body, "Network");
    if (info.ip4Address) addRow(netc, "IP Address", val(info.ip4Address), () => info.ip4Address);
    if (info.ip4Gateway) addRow(netc, "Gateway", val(info.ip4Gateway), () => info.ip4Gateway);
    if (info.ip4Dns) addRow(netc, "DNS", val(info.ip4Dns), () => info.ip4Dns);
  }

  forgetArmed = false;
  clearTimeout(forgetTimer);
  $("info-forget").textContent = "Forget";
  $("info-noprofile").classList.toggle("hidden", info.hasProfile);
  $("info-forget").classList.toggle("hidden", !info.hasProfile);
  $("info-save").classList.add("hidden");  // appears once the password is edited
  showView("info");
}

function closeInfoView() {
  info = null;
  infoNet = null;
  clearTimeout(forgetTimer);
  forgetArmed = false;
  showView("list");
}

function setInfoBusy(b, msg = "") {
  busy = b;
  for (const id of ["info-action", "info-back", "info-forget", "info-save"]) {
    $(id).disabled = b;
  }
  const s = $("info-status");
  s.classList.remove("error");
  s.textContent = msg;
}

// Run connect/disconnect from the info view, then re-render it with fresh state.
async function infoRun(ssid, progress, fn) {
  setInfoBusy(true, progress);
  let err = "";
  try { await fn(); } catch (e) { err = String(e); }
  busy = false;
  await refreshQuick();
  if (currentView() !== "info" || !info || info.ssid !== ssid) return;
  const net = networks.find((n) => n.ssid === ssid);
  if (net) {
    await openInfoView(net);
    if (err) {
      $("info-status").textContent = err;
      $("info-status").classList.add("error");
    }
  } else {
    closeInfoView();
    if (err) setStatus(err, { error: true, sticky: true });
  }
}

// ── Event wiring ──────────────────────────────────────────────────────────────
$("close-btn").addEventListener("click", () => appWindow.close());

$("scan-btn").addEventListener("click", doScan);

$("wifi-toggle").addEventListener("change", async (e) => {
  wifiEnabled = e.target.checked;
  if (wifiEnabled) {
    setStatus("Turning Wi-Fi on…", { sticky: true });
    render();
    try {
      await invoke("set_wifi_enabled", { enabled: true });
      setStatus("");
      await doScan();
    } catch (err) {
      setStatus(String(err), { error: true, sticky: true });
      render();
    }
  } else {
    networks = [];
    selectedSsid = null;
    setStatus("");
    render();
    invoke("set_wifi_enabled", { enabled: false }).catch(() => {});
  }
});

$("pw-cancel").addEventListener("click", () => {
  if (busy) return;
  connectingTo = null;
  showView("list");
});
$("pw-connect").addEventListener("click", submitPassword);
$("pw-input").addEventListener("input", updatePwButton);
$("pw-ssid").addEventListener("input", updatePwButton);
$("pw-input").addEventListener("keydown", (e) => {
  if (e.key === "Enter") submitPassword();
});
$("pw-ssid").addEventListener("keydown", (e) => {
  if (e.key === "Enter") $("pw-input").focus();
});
$("pw-show").addEventListener("change", (e) => {
  $("pw-input").type = e.target.checked ? "text" : "password";
});

$("info-back").addEventListener("click", closeInfoView);

$("info-action").addEventListener("click", () => {
  if (busy || !info) return;
  const ssid = info.ssid;
  const net = networks.find((n) => n.ssid === ssid) || infoNet;
  if (net.inUse) {
    infoRun(ssid, `Disconnecting…`, () => invoke("disconnect", { ssid }));
  } else if (info.hasProfile) {
    infoRun(ssid, `Connecting…`, () => invoke("connect_saved", { ssid }));
  } else if (net.secured) {
    openPasswordView(ssid, false);
  } else {
    infoRun(ssid, `Connecting…`, () => invoke("connect", { ssid, password: null }));
  }
});

$("info-forget").addEventListener("click", async () => {
  // Two-step confirm: first click arms, second click within 3s executes.
  if (!forgetArmed) {
    forgetArmed = true;
    $("info-forget").textContent = "Really forget?";
    forgetTimer = setTimeout(() => {
      forgetArmed = false;
      $("info-forget").textContent = "Forget";
    }, 3000);
    return;
  }
  clearTimeout(forgetTimer);
  const ssid = info.ssid;
  closeInfoView();
  try {
    await invoke("forget", { ssid });
    setStatus(`Forgot ${ssid}`);
    if (selectedSsid === ssid) selectedSsid = null;
  } catch (e) {
    setStatus(String(e), { error: true, sticky: true });
  }
  render();
  refreshQuick();
});

$("info-save").addEventListener("click", async () => {
  const ssid = info.ssid;
  const password = $("info-pw").value;
  closeInfoView();
  try {
    await invoke("save_password", { ssid, password });
    setStatus(`Password updated for ${ssid}`);
  } catch (e) {
    setStatus(String(e), { error: true, sticky: true });
  }
  render();
});

// ── Keyboard navigation ───────────────────────────────────────────────────────
document.addEventListener("keydown", (e) => {
  const view = currentView();
  if (view === "list") {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      if (!networks.length) return;
      e.preventDefault();
      const idx = networks.findIndex((n) => n.ssid === selectedSsid);
      let next;
      if (idx === -1) next = e.key === "ArrowDown" ? 0 : networks.length - 1;
      else next = Math.max(0, Math.min(networks.length - 1, idx + (e.key === "ArrowDown" ? 1 : -1)));
      selectedSsid = networks[next].ssid;
      renderList();
      renderChrome();
      document.querySelector("#network-list .row.selected")?.scrollIntoView({ block: "nearest" });
    } else if (e.key === "Enter" || e.key === "ArrowRight") {
      const sel = networks.find((n) => n.ssid === selectedSsid);
      if (sel && !busy) openInfoView(sel);
    } else if (e.key === "Escape") {
      appWindow.close();
    }
  } else if (view === "password") {
    if (e.key === "Escape" && !busy) {
      connectingTo = null;
      showView("list");
    }
  } else if (view === "info") {
    if (e.key === "Escape" || e.key === "ArrowLeft") closeInfoView();
  }
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

// Keep the list and signal strengths fresh while idle (no active rescan —
// this just re-reads NetworkManager's scan cache, which it refreshes itself).
setInterval(() => {
  if (currentView() === "list" && wifiEnabled && !scanning && !busy) refreshQuick();
}, 15000);
