// Genera una guía/kit honesto para capturar con Pwnagotchi (RPi, externo) y
// crackear en UIFIPILL. Sin ejecución remota, sin RF, sin fakes.
use crate::commands::CmdResponse;
use tauri::command;

fn sanitize(s: &str) -> String {
    s.trim()
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .collect::<String>()
        .replace('"', "'")
}

#[command]
pub fn gen_pwnagotchi_kit(
    ssid: Option<String>,
    bssid: Option<String>,
    channel: Option<u32>,
) -> CmdResponse {
    let ssid = sanitize(ssid.as_deref().unwrap_or(""));
    let bssid = sanitize(bssid.as_deref().unwrap_or(""));
    let ch = channel.unwrap_or(0);
    let ssid_label = if ssid.is_empty() { "TU_SSID_LAB".to_string() } else { ssid.clone() };
    let bssid_label = if bssid.is_empty() { "AA:BB:CC:DD:EE:FF".to_string() } else { bssid.clone() };
    let ch_label = if ch == 0 { "?".to_string() } else { ch.to_string() };

    let mut out = String::new();
    out.push_str("=== Kit Pwnagotchi -> UIFIPILL (captura externa + crack local) ===\n");
    out.push_str("LABORATORIO SOLO: redes propias o con autorizacion explicita.\n");
    out.push_str("Pwnagotchi corre en una Raspberry Pi (Linux) con su propio adaptador Wi-Fi en monitor;\n");
    out.push_str("NO se ejecuta desde UIFIPILL ni desde Windows y NO cambia tus drivers/antenas de Windows.\n\n");

    out.push_str("Objetivo (contexto):\n");
    out.push_str(&format!("  SSID : {}\n", ssid_label));
    out.push_str(&format!("  BSSID: {}\n", bssid_label));
    out.push_str(&format!("  Canal: {}\n\n", ch_label));

    out.push_str("1) Hardware/imagen (resumen)\n");
    out.push_str("   - Raspberry Pi Zero 2 W (recomendado) + microSD + alimentacion estable.\n");
    out.push_str("   - Opcional: pantalla e-Paper Waveshare 2.13\" (tipo v2/v3/v4 segun imagen).\n");
    out.push_str("   - Adaptador Wi-Fi con modo monitor (e idealmente inyeccion) compatible con tu imagen.\n");
    out.push_str("   - Imagen: proyecto original con mantenimiento limitado; forks activos (p.ej. jayofelony/pwnagotchi)\n");
    out.push_str("     ofrecen imagenes/docs mas recientes. Flashea la imagen adecuada a tu Pi/pantalla.\n\n");

    out.push_str("2) Configuracion minima (config.toml) - AJUSTA y pon SIEMPRE tu whitelist\n");
    out.push_str("   (ruta segun imagen, p.ej. /etc/pwnagotchi/config.toml)\n");
    out.push_str("   -----------------------------------------\n");
    out.push_str("   main.name = \"pwnagotchi\"\n");
    out.push_str("   main.lang = \"en\"\n");
    out.push_str(&format!("   # Whitelist: NO atacar estas redes (incluye tu casa/lab). Anade SSID/BSSID.\n   main.whitelist = [\"{}\", \"{}\"]\n", ssid_label, bssid_label));
    out.push_str("   ui.display.enabled = false          # pon true y el type correcto si tienes pantalla\n");
    out.push_str("   # ui.display.type = \"waveshare_v2\"  # v2/v3/v4 segun tu hardware\n");
    out.push_str("   ui.web.enabled = true\n");
    out.push_str("   ui.web.address = \"0.0.0.0\"\n");
    out.push_str("   ui.web.username = \"admin\"\n");
    out.push_str("   ui.web.password = \"CAMBIA-ESTO\"\n");
    out.push_str("   bettercap.username = \"pwnagotchi\"\n");
    out.push_str("   bettercap.password = \"CAMBIA-ESTO\"\n");
    out.push_str("   # Solo en tu laboratorio autorizado:\n");
    out.push_str("   personality.deauth = true\n");
    out.push_str("   personality.associate = true\n");
    out.push_str("   main.plugins.grid.enabled = false   # desactiva subidas externas si quieres offline\n");
    out.push_str("   -----------------------------------------\n\n");

    out.push_str("3) Captura\n");
    out.push_str("   - Arranca en modo AUTO/AI; el agente A2C aprende con el tiempo (epocas/recompensas) y guarda\n");
    out.push_str("     handshakes/PMKID en .pcap. Colocalo cerca del AP objetivo y en su canal (");
    out.push_str(&ch_label);
    out.push_str(") si es posible.\n");
    out.push_str("   - Rutas tipicas de handshakes (segun imagen): /root/handshakes/ o /home/pi/handshakes/\n");
    out.push_str("   - Copia los .pcap a este PC (ejemplo por USB gadget/IP de gestion):\n");
    out.push_str("       scp pi@10.0.0.2:/root/handshakes/*.pcap .\\handshakes\\\n\n");

    out.push_str("4) Convertir a .22000 (hashcat WPA-PBKDF2-PMKID+EAPOL, -m 22000)\n");
    out.push_str("   - Con hcxtools (referencia):\n");
    out.push_str("       hcxpcapngtool -o captura.22000 .\\handshakes\\*.pcap\n");
    out.push_str("   - En UIFIPILL: puedes convertir el .pcap con la herramienta nativa de conversion\n");
    out.push_str("     (pcap_to_22000) o indicar el .22000/.pcap y dejar que el plan lo use.\n\n");

    out.push_str("5) Crackear en UIFIPILL (este PC)\n");
    out.push_str("   - Pestaña Ataque: pon el .22000 (o .pcap) en el campo de hash (tarjeta avanzada oculta o flujo)\n");
    out.push_str("     y pulsa «Atacar» / «Probar contrasenas»: el plan ejecuta en orden\n");
    out.push_str("     keygen -> mascaras -> diccionarios/reglas -> PCFG (hashcat v8+, si disponible) ->\n");
    out.push_str("     candidatas detectadas en tools/ (p.ej. exports PassGAN/PassGPT *.txt) -> externo opcional,\n");
    out.push_str("     y se detiene solo con .cracked real. Sin hash/candidatas validas no hay exito (honesto).\n");
    out.push_str("   - WPA2 es lento por PBKDF2; WPA3-SAE puro no tiene crack offline equivalente.\n\n");

    out.push_str("Notas: esta guia es informativa y no ejecuta nada en la Pi. Verifica siempre\n");
    out.push_str("la whitelist y opera unicamente sobre redes de tu laboratorio.\n");

    CmdResponse {
        success: true,
        output: out,
        stderr: String::new(),
        exit_code: Some(0),
    }
}
