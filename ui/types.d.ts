// Shapes of the data crossing the Tauri IPC boundary. These mirror the
// #[derive(Serialize)] structs and #[tauri::command] signatures in
// src-tauri/src/main.rs (serde renames fields to camelCase, and Tauri
// converts camelCase argument keys to the snake_case Rust parameters).

interface Network {
  ssid: string;
  signal: number;
  secured: boolean;
  inUse: boolean;
  known: boolean;
}

interface WireguardStatus {
  installed: boolean;
  up: boolean;
  interface: string;
}

// What fetch_info returns; empty strings mean "unknown".
interface FetchedInfo {
  ssid: string;
  hasProfile: boolean;
  password: string;
  autoconnect: boolean | null;
  bssid: string;
  channel: string;
  band: string;
  rate: string;
  security: string;
  ip4Address: string;
  ip4Gateway: string;
  ip4Dns: string;
}

// The info view's state: fetch_info plus `secured`, which only the scan list knows.
interface InfoState extends FetchedInfo {
  secured: boolean;
}

// Every Tauri command: its argument object (null = takes none) and result type.
interface Commands {
  scan: { args: null; result: Network[] };
  quick_scan: { args: null; result: Network[] };
  get_wifi_enabled: { args: null; result: boolean };
  set_wifi_enabled: { args: { enabled: boolean }; result: null };
  connect: { args: { ssid: string; password: string | null }; result: null };
  connect_saved: { args: { ssid: string }; result: null };
  connect_hidden: { args: { ssid: string; password: string | null }; result: null };
  set_autoconnect: { args: { ssid: string; enabled: boolean }; result: null };
  disconnect: { args: { ssid: string }; result: null };
  forget: { args: { ssid: string }; result: null };
  save_password: { args: { ssid: string; password: string }; result: null };
  fetch_info: { args: { ssid: string; inUse: boolean }; result: FetchedInfo };
  wireguard_status: { args: null; result: WireguardStatus };
  set_wireguard: { args: { up: boolean; interface: string }; result: null };
}

// The pieces of the injected global Tauri API (withGlobalTauri) this app uses.
interface Window {
  __TAURI__: {
    core: {
      invoke(cmd: string, args?: Record<string, unknown>): Promise<unknown>;
    };
    window: {
      getCurrentWindow(): { close(): Promise<void> };
    };
  };
}
