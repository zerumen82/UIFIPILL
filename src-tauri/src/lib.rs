mod commands;
mod tools_detect;
mod wifi_adapter;
mod monitor_mode;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
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
            // tool detection
            tools_detect::detect_tools,
            tools_detect::find_tool_cmd,
            tools_detect::check_tool,
            // wifi adapter / chipset
            wifi_adapter::detect_adapters,
            wifi_adapter::set_monitor_mode,
            wifi_adapter::set_managed_mode,
            wifi_adapter::wsl2_info,
            // modo monitor nativo NPcap
            monitor_mode::activate_monitor,
            monitor_mode::restore_managed,
            monitor_mode::set_monitor_channel,
            monitor_mode::monitor_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
