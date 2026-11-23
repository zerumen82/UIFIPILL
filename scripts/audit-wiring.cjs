const fs = require('fs');
const path = require('path');

const ROOT = process.cwd();
const html = fs.readFileSync(path.join(ROOT, 'index.html'), 'utf8');
const jsFiles = [];
function walk(dir) {
  for (const f of fs.readdirSync(dir)) {
    const p = path.join(dir, f);
    const st = fs.statSync(p);
    if (st.isDirectory()) walk(p);
    else if (/\.(js|cjs|mjs)$/.test(f)) jsFiles.push(p);
  }
}
walk(path.join(ROOT, 'src'));
jsFiles.push(path.join(ROOT, 'index.html'));
const js = jsFiles.map(f => fs.readFileSync(f, 'utf8')).join('\n');
const lib = fs.readFileSync(path.join(ROOT, 'src-tauri/src/lib.rs'), 'utf8');

const invokes = new Set();
for (const m of js.matchAll(/__invoke\(\s*['"]([a-zA-Z0-9_]+)['"]/g)) invokes.add(m[1]);

const handlers = new Set();
for (const m of lib.matchAll(/([a-zA-Z0-9_]+)::([a-zA-Z0-9_]+)/g)) handlers.add(m[2]);

let bad = 0;
for (const c of invokes) {
  if (!handlers.has(c)) { console.error('ORPHAN invoke (no handler):', c); bad++; }
}
const onclicks = new Set();
for (const m of html.matchAll(/onclick="([a-zA-Z0-9_]+)\(/g)) onclicks.add(m[1]);
for (const fn of onclicks) {
  const defined = new RegExp('window\\.' + fn + '\\s*=').test(js) || new RegExp('function\\s+' + fn + '\\s*\\(').test(js) || new RegExp('async\\s+function\\s+' + fn + '\\s*\\(').test(js);
  if (!defined) { console.error('ORPHAN onclick (no definition):', fn); bad++; }
}
// ── Regresiones del botón Atacar / Auto ────────────────────────────────────
const appSrc = fs.readFileSync(path.join(ROOT, 'src/app.js'), 'utf8');

// simpleAttack: un 2º claimAttack veía _attackBusy=true (puesto por
// setAttackRunning) y hacía return ANTES del try/finally → lock pegado y el
// botón Atacar parecía no hacer nada. Debe haber exactamente UNO.
const simpleClaims = (appSrc.match(/claimAttack\(\s*'simple_attack'\s*\)/g) || []).length;
if (simpleClaims !== 1) {
  console.error(`REGRESSION: claimAttack('simple_attack') x${simpleClaims} (esperado 1) — el botón Atacar se queda colgado.`);
  bad++;
}

// `activeNet` se leía en la 1ª línea de simpleAttack sin estar declarado en
// ningún sitio → ReferenceError antes del try → el clic no hacía NADA.
const appCode = appSrc.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/.*$/gm, '');
if (/\bactiveNet\b/.test(appCode)) {
  console.error("REGRESSION: identificador `activeNet` sin declarar en app.js — el botón Atacar muere con ReferenceError.");
  bad++;
}

// bssid/ssid/ch se declaraban con `let` DENTRO del try y el finally los leía
// para addAttackResult → ReferenceError al terminar cada ataque. Deben estar
// declarados antes del `try {` de simpleAttack.
const saStart = appSrc.indexOf('window.simpleAttack = async function () {');
const saTry = appSrc.indexOf('\n  try {', saStart);
if (saStart >= 0 && saTry > saStart) {
  const preTry = appSrc.slice(saStart, saTry);
  for (const name of ['bssid', 'ssid', 'ch']) {
    if (!new RegExp('let\\s+' + name + '\\b').test(preTry)) {
      console.error(`REGRESSION: \`${name}\` no declarado antes del try de simpleAttack — el finally lanza ReferenceError.`);
      bad++;
    }
  }
}

// startAutoAttack pone el lock de UI para toda la secuencia; invokeAttack
// rechaza por busy salvo que el paso pase ownsLock:true. Si un paso lo omite,
// el botón Auto no ejecuta nada.
const autoStart = appSrc.indexOf('window.startAutoAttack = async function () {');
const autoEnd = appSrc.indexOf('// Helper: espera a que termine un ataque bg', autoStart);
if (autoStart >= 0 && autoEnd > autoStart) {
  const autoBody = appSrc.slice(autoStart, autoEnd);
  for (const m of autoBody.matchAll(/invokeAttack\([^)]*\)/g)) {
    if (!/ownsLock:\s*true/.test(m[0])) {
      console.error('REGRESSION: invokeAttack() en startAutoAttack sin ownsLock:true — los pasos se rechazan por el lock.', m[0]);
      bad++;
    }
  }
}

// El gatillo EAPOL (dos antenas) se eliminó en 1.0.12 de UI, JS y backend.
// No debe volver a aparecer como código muerto.
const htmlCode = html.replace(/<!--[\s\S]*?-->/g, '');
const strippedApp = appSrc.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/.*$/gm, '');
if (/wpa2_trigger|triggerEapol|triggerCancel|simple-trigger-card/.test(strippedApp + htmlCode)) {
  console.error('REGRESSION: el gatillo EAPOL vuelve a estar cableado (UI/JS) — se eliminó en 1.0.12.');
  bad++;
}
if (/\bmod\s+wpa2_trigger\b|wpa2_trigger::/.test(lib)) {
  console.error('REGRESSION: el backend wpa2_trigger vuelve a estar registrado en lib.rs — se eliminó en 1.0.12.');
  bad++;
}

// Tauri inyecta hashes sha256 en el CSP declarado en tauri.conf.json. En cuanto
// hay hashes, el navegador IGNORA 'unsafe-inline' y bloquea TODOS los
// onclick="..." del HTML (Atacar, Detener, pestañas...): medido 2026-11-22 en
// el WebView con CDP. La app es lab-only y no carga contenido remoto, así que
// el CSP se desactiva explícitamente (csp: null). No volver a poner un string.
const tauriConf = JSON.parse(fs.readFileSync(path.join(ROOT, 'src-tauri', 'tauri.conf.json'), 'utf8'));
if (tauriConf?.app?.security?.csp !== null) {
  console.error('REGRESSION: app.security.csp no es null en tauri.conf.json — Tauri inyecta hashes y bloquea los onclick inline (botones muertos).');
  bad++;
}
if (/http-equiv=["']Content-Security-Policy["']/i.test(html)) {
  console.error('REGRESSION: vuelve a haber una meta CSP en index.html — bloquea los onclick inline y el IPC (ipc.localhost) de Tauri.');
  bad++;
}
if (/alert\(\s*['"]click['"]\s*\)/.test(html)) {
  console.error('REGRESSION: queda un alert de diagnóstico en un onclick de index.html — bloquea el clic en la app.');
  bad++;
}

// ── E0-18: Modo IA (tool-calling total) ────────────────────────────────────
const iaSrc = fs.readFileSync(path.join(ROOT, 'src', 'js', 'ia.js'), 'utf8');
const cmdsSrc = fs.readFileSync(path.join(ROOT, 'src-tauri', 'src', 'commands.rs'), 'utf8');
if (!/id="ia-mode-toggle"/.test(html)) {
  console.error('REGRESSION: falta el interruptor #ia-mode-toggle en index.html (Modo IA E0-18).'); bad++;
}
if (!/window\._iaMode\s*=\s*false/.test(iaSrc)) {
  console.error('REGRESSION: _iaMode no arranca en false — el ataque pasaría a modo IA sin querer.'); bad++;
}
if (!/if \(window\._iaMode\) return window\.iaAttack\(\);/.test(appSrc)) {
  console.error('REGRESSION: simpleAttack no deriva al Modo IA cuando está activo.'); bad++;
}
if (!/window\.iaAttack\s*=\s*async function/.test(appSrc) || !/IA_TOOL_SPECS/.test(iaSrc)) {
  console.error('REGRESSION: falta la ejecución IA o su lista blanca (E0-18).'); bad++;
}
if (!/GUID real inyectado/.test(appSrc) || !/ctx\.ssid/.test(appSrc)) {
  console.error('REGRESSION: falta la inyección de GUID/SSID reales en los pasos de la IA (los marcadores `?` rompen el plan).'); bad++;
}
if (!/Plan SIN RESULTADOS/.test(appSrc)) {
  console.error('REGRESSION: veredicto deshonesto — el plan debe decir CUÁNTOS pasos fallaron.'); bad++;
}
if (!/window\._attackAwaitingConfirm/.test(appSrc) || !/window\._attackAwaitingConfirm/.test(iaSrc)) {
  console.error('REGRESSION: el watchdog no se pausa mientras espera confirmación (lock liberado a los 10 min).'); bad++;
}
if (!/hasAction\(\)/.test(appSrc) || !/paso\(s\) mínimo\(s\) con herramientas IA/.test(appSrc)) {
  console.error('REGRESSION: falta el plan mínimo obligatorio del Modo IA tras un plan corto.'); bad++;
}

const toolsBlock = cmdsSrc.slice(cmdsSrc.indexOf('let tools = serde_json::json!(['), cmdsSrc.indexOf(']);', cmdsSrc.indexOf('let tools = serde_json::json!([')));

// ── E0-20 fase 1: gate PMF + read-back de canal TX ──────────────────────────
if (!/id="pmf-gate-note"/.test(html) || !/window\._pmfBlocked/.test(appSrc)) {
  console.error('REGRESSION: falta el gate PMF visible/bloqueante en la UI (E0-20).'); bad++;
}
if (!/checkPmfGate/.test(appSrc) || !/__invoke\('parse_pmf'/.test(appSrc)) {
  console.error('REGRESSION: no se consulta el RSN IE antes de atacar (E0-20).'); bad++;
}
const usbRawSrc = fs.readFileSync(path.join(ROOT, 'src-tauri', 'src', 'usb_raw.rs'), 'utf8');
const usbTxToml = fs.readFileSync(path.join(ROOT, 'driver_re', 'usb_tx', 'Cargo.toml'), 'utf8');
if (!/usb_raw_tx_preflight/.test(lib) || !/rt3070_chan_preflight/.test(usbRawSrc) || !/rt3070_chan_preflight/.test(usbTxToml)) {
  console.error('REGRESSION: falta el preflight de canal TX con read-back (E0-20).'); bad++;
}
if (!/capture_until_eapol/.test(lib) || !/eapol-detected/.test(appSrc) || !/window\.iaDeauthBurst/.test(appSrc)) {
  console.error('REGRESSION: falta el watcher EAPOL o la ráfaga deauth (E0-20 fase 2).'); bad++;
}
if (!/deauth_burst/.test(toolsBlock) || !/deauth_burst/.test(iaSrc)) {
  console.error('REGRESSION: deauth_burst no está ofrecido/validado para la IA.'); bad++;
}
if (!/crack_backend_check/.test(lib) || !/crack_backend_check/.test(appSrc)) {
  console.error('REGRESSION: falta el preflight OpenCL al arrancar (D-07).'); bad++;
}
// E0-22: generador de diccionarios con fichero real
if (!/generate_dictionary/.test(lib) || !/generate_dictionary/.test(iaSrc) || !/generate_dictionary/.test(toolsBlock)) {
  console.error('REGRESSION: falta la herramienta generadora de diccionarios (E0-22).'); bad++;
}
if (!/ia_memory_list/.test(lib) || !/iaMemoryContext/.test(iaSrc) || !/iaMemoryContext/.test(appSrc)) {
  console.error('REGRESSION: falta la memoria local de la IA inyectada en el prompt (E0-23).'); bad++;
}
// E0-24: auto-gatillo M1/M2
if (!/wifi_probe_trigger/.test(lib) || !/convert_ethernet_eapol/.test(lib) || !/iaTriggerHandshake/.test(appSrc)) {
  console.error('REGRESSION: falta el auto-gatillo M1/M2 o su conversor (E0-24).'); bad++;
}
// E0-25: PMKID provocado por Association Request
if (!/usb_raw_assoc/.test(lib) || !/convert_assoc_pmkid/.test(lib) || !/iaPmkidProbe/.test(appSrc) || !/rt3070_assoc/.test(usbTxToml)) {
  console.error('REGRESSION: falta el provocador de PMKID por USB crudo (E0-25).'); bad++;
}
if (!tauriConf?.bundle?.resources?.some(r => /rt3070_deauth\.exe/.test(r))) {
  console.error('REGRESSION: el instalador no incluye rt3070_deauth.exe (la IA no podría transmitir).'); bad++;
}
if (/"name":"(deauth|wps_pin)"/.test(toolsBlock)) {
  console.error('REGRESSION: deauth/wps_pin ofrecidos como herramientas de IA — Npcap no inyecta en Windows (medido).'); bad++;
}
if (!/capture_packets/.test(toolsBlock) || !/restore_managed/.test(toolsBlock)) {
  console.error('REGRESSION: faltan herramientas del pipeline en ollama_tools (E0-18).'); bad++;
}

// ── E0-19: Consola con tool-calling real ────────────────────────────────────
if (!/t\.tool_calls/.test(iaSrc) || !/window\.iaConsoleAsk\s*=\s*async function/.test(iaSrc)) {
  console.error('REGRESSION: la Consola vuelve a ignorar tool_calls (caerá en chat y el modelo se negará).'); bad++;
}
if (!/window\._iaConsoleTools/.test(iaSrc)) {
  console.error('REGRESSION: falta la confirmación interactiva de la Consola (E0-19).'); bad++;
}
if (!/Contexto: laboratorio WiFi/.test(iaSrc)) {
  console.error('REGRESSION: falta el contexto lab-only en el chat de la Consola (E0-19).'); bad++;
}

if (bad) { console.error('audit: ' + bad + ' issue(s)'); process.exit(1); }
console.log('audit-wiring: OK (' + invokes.size + ' invokes, ' + onclicks.size + ' onclicks)');
