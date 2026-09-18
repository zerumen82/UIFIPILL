mod commands;
mod capture;
mod connect;
mod crack;
mod eviltwin;
mod keygen;
mod profiler;
mod tools_detect;
mod wifi_adapter;
mod monitor_mode;
mod pcap_convert;
mod wpa3;
mod wsl;

use std::collections::HashMap;
use std::sync::Mutex;
use tauri_plugin_shell::process::CommandChild;

/// Emite stdout+stderr troceados como eventos attack-progress (wrapper para
/// wsl::wsl_stream_run; reutiliza emit_chunked de commands).
pub(crate) fn emit_stream(app: &tauri::AppHandle, id: &str, out: &str, err: &str) {
    commands::emit_chunked(app, id, "stdout", out);
    commands::emit_chunked(app, id, "stderr", err);
}

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
            commands::wash_scan,
            commands::wash_scan_bg,
            commands::wps_bruteforce_reaver,
            commands::wps_bruteforce_reaver_bg,
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
            monitor_mode::set_monitor_freq,
            monitor_mode::monitor_status,
            // captura nativa Npcap (wpcap.dll, sin binarios externos)
            capture::native_capture,
            capture::check_injection_capability,
            // pcap converter nativo (reemplaza hcxpcapngtool)
            pcap_convert::pcap_to_22000,
            // perfilador de objetivo (solo lectura)
            profiler::profile_target,
            // estrategia de crack (hashcat -m 22000, corre en Windows)
            crack::inspect_hash,
            crack::filter_hash,
            crack::list_crack_assets,
            crack::crack_custom,
            // WPA3: auditoría + rogue conf (sin ejecución en Windows)
            wpa3::wpa3_audit,
            wpa3::gen_rogue_conf,
            // Evil Twin: kit de lab + verificación de 1 candidato (hashcat local)
            eviltwin::gen_eviltwin_kit,
            eviltwin::verify_candidate,
            // Conexión a la red atacada (lab): perfil netsh + keygen Comtrend
            connect::wifi_connect,
            connect::wifi_disconnect,
            keygen::keygen_detect,
            keygen::keygen_run,
            keygen::thomson_run,
            // background + cancel
            commands::cancel_attack,
            commands::pmkid_capture_bg,
            commands::capture_handshake_bg,
            commands::scan_airodump_bg,
            commands::wps_pin_bruteforce_bg,
            commands::wps_pbc_attack_bg,
            commands::wps_pixiedust_bg,
            commands::cleanup_temp_files,
            // puente WSL2/Kali (motor RF dual, ROADMAP_WSL2 Fase 5)
            wsl::wsl_exec,
            wsl::wsl_attach,
            wsl::wsl_detach,
            wsl::wsl_from_win,
            wsl::wsl_to_win,
            wsl::wsl_is_available,
            wsl::wsl_health,
            wsl::wsl_pmkid_stream,
            wsl::wsl_airodump_stream,
            wsl::wsl_deauth_stream,
            wsl::wsl_pmkid_capture,
            wsl::wsl_airodump,
            wsl::wsl_wash,
            wsl::wsl_deauth,
            wsl::wsl_reaver,
            wsl::wsl_kill,
            // VirtualHere: transporte USB alternativo a usbipd (Fase 6b)
            wsl::vh_server_check,
            wsl::vh_provision,
            wsl::vh_daemon,
            wsl::vh_hub_add,
            wsl::vh_list,
            wsl::vh_use,
            wsl::vh_stop,
            wsl::vh_rf_check,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
