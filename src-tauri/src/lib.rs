mod commands;
mod crack;
mod tools_detect;
mod wifi_adapter;
mod monitor_mode;
mod pcap_convert;
mod profiler;

use std::collections::HashMap;
use std::sync::Mutex;
use tauri_plugin_shell::process::CommandChild;

pub struct AppState {
    pub running_attacks: Mutex<HashMap<String, CommandChild>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            running_attacks: Mutex::new(HashMap::new()),
        })
        .invoke_handler(tauri::generate_handler![
            // commands
            commands::scan_wifi,
            commands::pmkid_capture,
            commands::pmkid_convert,
            commands::pmkid_crack,
            commands::wps_pin_bruteforce,
            commands::wps_pbc_attack,
            commands::scan_airodump,
            commands::wifi_interface_status,
            commands::list_attack_processes,
            commands::kill_attack_process,
            commands::capture_handshake,
            commands::crack_handshake,
            commands::deauth_inject,
            commands::disassoc_inject,
            commands::arp_replay_inject,
            commands::beacon_flood,
            commands::chopchop_inject,
            commands::rogue_ap,
            commands::injection_test,
            commands::fakeauth_inject,
            commands::wps_pixiedust,
            commands::cafe_latte_attack,
            commands::interactive_inject,
            commands::fragment_inject,
            // tool detection
            tools_detect::detect_tools,
            tools_detect::find_tool_cmd,
            tools_detect::check_tool,
            // wifi adapter / chipset
            wifi_adapter::detect_adapters,
            wifi_adapter::set_monitor_mode,
            wifi_adapter::set_managed_mode,

            // modo monitor nativo NPcap
            monitor_mode::activate_monitor,
            monitor_mode::restore_managed,
            monitor_mode::set_monitor_channel,
            monitor_mode::monitor_status,
            // pcap converter nativo (reemplaza hcxpcapngtool)
            pcap_convert::pcap_to_22000,
            // perfilador de objetivo (solo lectura)
            profiler::profile_target,
            // estrategia de crack (hashcat -m 22000, corre en Windows)
            crack::inspect_hash,
            crack::filter_hash,
            crack::list_crack_assets,
            crack::crack_custom,
            // background + cancel
            commands::cancel_attack,
            commands::pmkid_capture_bg,
            commands::capture_handshake_bg,
            commands::scan_airodump_bg,
            commands::wps_pin_bruteforce_bg,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
