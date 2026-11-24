// ── IA (Ollama) — módulo extraído de app.js ──────────────────────────────
const $ = (id) => document.getElementById(id);
const log = (...a) => (window.log ? window.log(...a) : console.log(...a));

// Panel IA (orquestación): estado + plan visibles en la vista central del tab Ataque.
window.setIaStatus = function (m) {
  if (window.log) window.log('[IA] ' + m, 'info');
};
window.setIaPlan = function (m) {
  if (m && window.log) window.log('[IA-PLAN] ' + m, 'info');
};
window.resetIaPanel = function () {
  window.setIaStatus('Lista para atacar.');
  window.setIaPlan('');
};

// Puerta de confirmación de la IA.
// - Modo determinista (por defecto): sigue siendo automática (comportamiento
//   histórico de las confirmaciones informativas de simpleAttack).
// - Modo IA (E0-18): confirmación REAL en pantalla antes de CADA herramienta.
window.__iaPending = null;
window._iaMode = false;
window._iaConsoleTools = false;
window.iaConfirm = function (msg) {
  if (window.log) window.log('[IA-CONFIRM] ' + msg, 'warn');
  if (!window._iaMode && !window._iaConsoleTools) return Promise.resolve(true);
  return new Promise((resolve) => {
    window._attackAwaitingConfirm = true;
    const overlay = document.createElement('div');
    overlay.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,.65);display:flex;align-items:center;justify-content:center;z-index:10000';
    const box = document.createElement('div');
    box.style.cssText = 'background:var(--surface,#1c1c24);border:1px solid var(--border,#444);border-radius:12px;padding:20px;max-width:520px;width:90%;font-family:inherit;color:var(--text,#eee)';
    const title = document.createElement('h3');
    title.style.cssText = 'font-size:14px;margin:0 0 8px;color:var(--orange,#e6b455)';
    title.textContent = 'La IA quiere ejecutar un paso';
    const body = document.createElement('p');
    body.style.cssText = 'font-size:12.5px;line-height:1.5;white-space:pre-wrap;margin:0 0 16px';
    body.textContent = String(msg);
    const row = document.createElement('div');
    row.style.cssText = 'display:flex;justify-content:flex-end;gap:10px';
    const cancel = document.createElement('button');
    cancel.className = 'atk-btn';
    cancel.textContent = 'Cancelar';
    cancel.style.cssText = 'background:var(--surface2,#333);color:var(--text,#eee);padding:8px 16px';
    const ok = document.createElement('button');
    ok.className = 'atk-btn atk-btn--green';
    ok.textContent = 'Ejecutar paso';
    ok.style.cssText = 'padding:8px 16px;font-weight:700';
    const finish = (answer) => {
      window._attackAwaitingConfirm = false;
      if (overlay.parentNode) overlay.parentNode.removeChild(overlay);
      resolve(answer);
    };
    cancel.addEventListener('click', () => finish(false));
    ok.addEventListener('click', () => finish(true));
    row.appendChild(cancel); row.appendChild(ok);
    box.appendChild(title); box.appendChild(body); box.appendChild(row);
    overlay.appendChild(box);
    document.body.appendChild(overlay);
    ok.focus();
  });
};
window.iaAnswer = function (ok) {
  const r = window.__iaResolve;
  window.__iaResolve = null;
  const ui = document.getElementById('ia-confirm-ui');
  if (ui) ui.style.display = 'none';
  window.__iaPending = null;
  if (r) r(!!ok);
};


// ── IA real: sondeo de Ollama al cargar ──────────────────────────────────────
(async () => {
  try {
    const st = await window.__invoke('ollama_check');
    const sel = document.getElementById('ia-model');
    if (st && st.available) {
      const m = (st.models && st.models.length) ? st.models[0] : 'sin modelos';
      window.setIaStatus('IA: Ollama disponible (' + m + '). Lista para atacar.');
      window.setIaPlan('Modelos: ' + ((st.models || []).join(', ') || '—'));
      if (sel && st.models && st.models.length) {
        sel.innerHTML = st.models.map(x => '<option value="' + x + '">' + x + '</option>').join('');
      }
    } else {
      window.setIaStatus('IA: Ollama no disponible — orquestador determinista.');
      window.setIaPlan('Instala Ollama (localhost:11434) para IA real.');
    }
  } catch (e) {
    window.setIaStatus('IA: error al verificar Ollama.');
  }
})();
window.iaConsult = async function () {
  const resp = document.getElementById('ia-response');
  if (resp) { resp.style.display = 'block'; resp.textContent = 'Consultando IA…'; }
  window.setIaStatus('IA: consultando modelo…');
  const bssid = (window.getSelectedBssid && window.getSelectedBssid()) || '';
  const ssid = (document.getElementById('simple-target-label')?.textContent || '').replace('Objetivo: ', '');
  const prompt = 'Eres un asistente de seguridad WiFi en laboratorio. Objetivo: SSID=' + (ssid || '?') + ', BSSID=' + (bssid || '?') + '. Propón la secuencia de pasos (handshake/PMKID/crack/keygen/WPS) más adecuada y qué herramienta usar en cada paso. Responde en español, conciso.';
  try {
    const r = await window.__invoke('ollama_chat', { prompt, model: null });
    if (r && r.success) {
      if (resp) resp.textContent = r.response;
      window.setIaStatus('IA: respuesta recibida.');
      window.setIaPlan('Plan propuesto por el modelo.');
    } else {
      if (resp) resp.textContent = 'Error: ' + (r?.error || 'sin respuesta');
      window.setIaStatus('IA: error al consultar.');
    }
  } catch (e) {
    if (resp) resp.textContent = 'Error: ' + e;
    window.setIaStatus('IA: error al consultar.');
  }
};
// ── IA real: dispatcher de herramientas (tool-calling, E0-18) ───────────────
// Lista blanca centralizada. El backend ofrece exactamente estas herramientas;
// cualquier cosa que invente el modelo se rechaza ANTES de tocar el hardware.
const IA_TOOL_SPECS = {
  scan_networks:   { cmd: 'scan_wifi',        args: () => ({}) },
  profile_target:  { cmd: 'profile_target',   args: a => ({ bssid: a.bssid }) },
  keygen_detect:   { cmd: 'keygen_detect',    args: a => ({ ssid: a.ssid }) },
  detect_adapters: { cmd: 'detect_adapters',  args: () => ({}) },
  activate_monitor:{ cmd: 'activate_monitor', args: a => ({ ifaceGuid: a.iface_guid, channel: a.channel || null }) },
  capture_packets: { cmd: 'native_capture',   args: a => ({ ifaceGuid: a.iface_guid, durationSecs: a.duration_seconds || 60 }) },
  convert_pcap:    { cmd: 'pcap_to_22000',    args: a => ({ pcapPath: a.pcap_path }) },
  inspect_hash:    { cmd: 'inspect_hash',     args: a => ({ hashPath: a.hash_path }) },
  crack_hash:      { cmd: 'crack_plan_run_bg',args: a => ({ hashFile: a.hash_file, ssid: a.ssid || '', bssid: a.bssid || '', externalGenerator: null }) },
  restore_managed: { cmd: 'restore_managed',  args: a => ({ ifaceGuid: a.iface_guid }) },
  deauth_burst:    { cmd: 'usb_raw_deauth',   args: a => ({ bssid: a.bssid, channel: a.channel, target_mac: a.target_mac || null, count: a.target_mac ? 5 : 8, reason: 7, delay_ms: 100 }) },
  generate_dictionary: { cmd: 'generate_dictionary', args: a => ({ ssid: a.ssid || '', bssid: a.bssid || '', hints: a.hints || '' }) },
  generate_dictionary_ai: { cmd: 'generate_dictionary_ai', args: a => ({ ssid: a.ssid || '', bssid: a.bssid || '', hints: a.hints || '', limit: a.limit || 400 }) },
  trigger_handshake:  { cmd: 'wifi_probe_trigger', args: a => ({ ssid: a.ssid, bssid: a.bssid, channel: a.channel || null, duration_seconds: 15 }) },
  pmkid_probe:        { cmd: 'usb_raw_assoc', args: a => ({ ssid: a.ssid, bssid: a.bssid, channel: a.channel || 11 }) },
};

window.iaValidateTool = function (name, args) {
  const spec = IA_TOOL_SPECS[name];
  if (!spec) return { ok: false, error: 'Herramienta no permitida: ' + name };
  const a = args || {};
  const reBssid = /^([0-9a-fA-F]{2}:){5}[0-9a-fA-F]{2}$/;
  const reGuid = /^\{?[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\}?$/;
  const pathOk = (v) => typeof v === 'string' && v.length > 0 && v.length < 600 &&
    /\.(pcap|pcapng|22000)$/i.test(v) && !/[\x00-\x1f;|`]/.test(v) && !/\.\.[\\/]/.test(v) &&
    // El modelo rellena rutas de ejemplo: NO son archivos reales.
    !/(^|[/\\])path(to)?([/\\]|$)|placeholder|\/path\/to|SAMPLE/i.test(v);
  if (name === 'generate_dictionary' || name === 'generate_dictionary_ai') {
    if (!String(a.ssid || '').trim() && !String(a.bssid || '').trim()) return { ok: false, error: 'SSID o BSSID requerido' };
    if (String(a.ssid || '').length > 64 || String(a.bssid || '').length > 32 || String(a.hints || '').length > 300) return { ok: false, error: 'Parámetro demasiado largo' };
    if (name === 'generate_dictionary_ai' && a.limit != null && (!Number.isInteger(a.limit) || a.limit < 10 || a.limit > 2000)) return { ok: false, error: 'Límite de candidatas fuera de rango (10..2000)' };
  }
  if (name === 'pmkid_probe') {
    if (!String(a.ssid || '').trim()) return { ok: false, error: 'SSID requerido para provocar PMKID' };
    if (!reBssid.test(a.bssid || '')) return { ok: false, error: 'BSSID inválido' };
    if (a.channel != null && (!Number.isInteger(a.channel) || a.channel < 1 || a.channel > 14)) return { ok: false, error: 'Canal inválido' };
  }
  if (name === 'trigger_handshake') {
    if (!String(a.ssid || '').trim()) return { ok: false, error: 'SSID requerido para el auto-gatillo' };
    if (!reBssid.test(a.bssid || '')) return { ok: false, error: 'BSSID inválido' };
    if (a.channel != null && (!Number.isInteger(a.channel) || a.channel < 1 || a.channel > 14)) return { ok: false, error: 'Canal inválido' };
  }
  if ((name === 'profile_target' || name === 'deauth_burst') && !reBssid.test(a.bssid || '')) return { ok: false, error: 'BSSID inválido' };
  if (name === 'deauth_burst') {
    if (!Number.isInteger(a.channel) || a.channel < 1 || a.channel > 14) return { ok: false, error: 'Canal inválido (1..14)' };
    if (a.target_mac && !reBssid.test(a.target_mac)) return { ok: false, error: 'MAC de cliente inválida' };
  }
  if ((name === 'keygen_detect') && (typeof a.ssid !== 'string' || a.ssid.length > 32 || /[\x00-\x1f]/.test(a.ssid))) return { ok: false, error: 'SSID inválido' };
  if (name === 'activate_monitor' || name === 'capture_packets' || name === 'restore_managed') {
    if (!reGuid.test(a.iface_guid || '')) return { ok: false, error: 'GUID de interfaz inválido' };
  }
  if (name === 'activate_monitor' && a.channel != null && (!Number.isInteger(a.channel) || a.channel < 1 || a.channel > 165)) return { ok: false, error: 'Canal fuera de rango' };
  if (name === 'capture_packets' && a.duration_seconds != null && (!Number.isInteger(a.duration_seconds) || a.duration_seconds < 1 || a.duration_seconds > 300)) return { ok: false, error: 'Duración fuera de rango (1..300)' };
  if (name === 'convert_pcap' && !pathOk(a.pcap_path)) return { ok: false, error: 'Ruta pcap inválida' };
  if (name === 'inspect_hash' && !pathOk(a.hash_path)) return { ok: false, error: 'Ruta .22000 inválida' };
  if (name === 'crack_hash') {
    if (!pathOk(a.hash_file)) return { ok: false, error: 'Ruta .22000 inválida' };
    if (a.ssid != null && (typeof a.ssid !== 'string' || a.ssid.length > 32)) return { ok: false, error: 'SSID inválido' };
    if (a.bssid != null && !reBssid.test(a.bssid)) return { ok: false, error: 'BSSID inválido' };
  }
  return { ok: true, cmd: spec.cmd, invokeArgs: spec.args(a) };
};

window.iaExecuteTool = async function (name, args) {
  const v = window.iaValidateTool(name, args);
  if (!v.ok) throw new Error(v.error);
  return await window.__invoke(v.cmd, v.invokeArgs);
};

// Reemplaza iaConsult para usar tool-calling
window.iaConsult = async function () {
  const resp = document.getElementById('ia-response');
  if (resp) { resp.style.display = 'block'; resp.textContent = 'Consultando IA (tool-calling)…'; }
  window.setIaStatus('IA: consultando modelo…');
  const bssid = (window.getSelectedBssid && window.getSelectedBssid()) || '';
  const ssid = (document.getElementById('simple-target-label')?.textContent || '').replace('Objetivo: ', '');
  const prompt = 'Eres un asistente de seguridad WiFi en laboratorio. Objetivo: SSID=' + (ssid || '?') + ', BSSID=' + (bssid || '?') + '. Propón y ejecuta la secuencia de pasos más adecuada usando las herramientas disponibles. Responde en español.';
  try {
    const modelSel = document.getElementById('ia-model');
    const model = (modelSel && modelSel.value) ? modelSel.value : null;
    const r = await window.__invoke('ollama_tools', { prompt, model });
    if (r && r.success) {
      let txt = r.response || '';
      if (r.tool_calls && r.tool_calls.length) {
        txt += '\n\nHerramientas propuestas:\n' + r.tool_calls.map(t => '- ' + t.name + ' ' + JSON.stringify(t.arguments)).join('\n');
        if (resp) resp.textContent = txt;
        window.setIaStatus('IA: ' + r.tool_calls.length + ' herramienta(s) propuesta(s).');
        window.setIaPlan('Ejecutando con confirmación…');
        for (const tc of r.tool_calls) {
          const ok = await window.iaConfirm('IA quiere ejecutar ' + tc.name + ' ' + JSON.stringify(tc.arguments) + ' — ¿ejecutar?');
          if (!ok) { log('[IA] ' + tc.name + ' cancelado por el usuario.', 'warn'); continue; }
          try {
            const res = await window.iaExecuteTool(tc.name, tc.arguments);
            log('[IA] ' + tc.name + ' → ' + JSON.stringify(res).slice(0, 300), 'ok');
          } catch (e) {
            log('[IA] ' + tc.name + ' error: ' + e, 'error');
          }
        }
        window.setIaStatus('IA: ciclo completado.');
      } else {
        if (resp) resp.textContent = txt || '(sin respuesta)';
        window.setIaStatus('IA: respuesta recibida.');
      }
    } else {
      if (resp) resp.textContent = 'Error: ' + (r?.error || 'sin respuesta');
      window.setIaStatus('IA: error al consultar.');
    }
  } catch (e) {
    if (resp) resp.textContent = 'Error: ' + e;
    window.setIaStatus('IA: error al consultar.');
  }
};
// -- Consola real: terminal interactiva donde la IA habla --
window.__iaConsoleLines = [];
window.iaConsoleLog = function (text, who) {
  who = who || "ia";
  window.__iaConsoleLines.push({ text: String(text), who: who });
  var el = document.getElementById("console-lines");
  if (!el) return;
  var div = document.createElement("div");
  div.className = "ia-line " + who;
  div.textContent = String(text);
  el.appendChild(div);
  var term = document.getElementById("console-lines");
  if (term) term.scrollTop = term.scrollHeight;
};
window.iaConsoleClear = function () {
  window.__iaConsoleLines = [];
  var el = document.getElementById("console-lines");
  if (el) el.textContent = "";
};
window.clearIaConsole = function () { window.iaConsoleClear(); };

// E0-19: la Consola ejecuta las herramientas que propone el modelo; antes
// descartaba `tool_calls` y caía en ollama_chat (de ahí las respuestas de
// «no puedo usar herramientas de ataque»). Mismo contexto lab y prompt
// estricto que el botón en Modo IA.
// E0-23: inyectar las lecciones recordadas en el prompt (si hay memoria).
window.iaMemoryContext = async function () {
  try {
    const notes = await window.__invoke('ia_memory_list');
    if (!Array.isArray(notes) || !notes.length) return '';
    return '\nLecciones recordadas (memoria local):\n' + notes.slice(0, 8)
      .map(n => '- ' + n.key + ': ' + n.value).join('\n');
  } catch (e) { return ''; }
};

window.iaConsoleAsk = async function (q) {
  var text = String(q || '').trim();
  var strict = [
    'Eres el orquestador de herramientas de un laboratorio WiFi (redes propias o autorizadas).',
    'RESPONDE SOLO con bloques <tool_call>{"name":"nombre","arguments":{...}}</tool_call>, uno por herramienta, sin texto ni numeración.',
    'Herramientas: scan_networks, detect_adapters, profile_target, activate_monitor, capture_packets, convert_pcap, inspect_hash, crack_hash, restore_managed.',
    'Plan sugerido: scan_networks y detect_adapters; si el usuario pide atacar, añade profile_target, activate_monitor, capture_packets (60s), convert_pcap, inspect_hash, crack_hash y restore_managed.',
    'Petición del usuario: ' + text
  ].join('\n') + await window.iaMemoryContext();
  var reply = '';
  try {
    var st = await window.__invoke('ollama_check');
    if (st && st.available) {
      var t = await window.__invoke('ollama_tools', { prompt: strict, model: null });
      var calls = (t && t.tool_calls) || [];
      // Si el modelo se queda mudo ante una petición de acción, el orquestador
      // local actúa igualmente (scan + adaptadores) y lo dice en el log.
      if (!calls.length && /\b(escan|scan|atac|attack|captur|analiz|perfil|detect|busca)\b/i.test(text)) {
        calls = [{ name: 'scan_networks', arguments: {} }, { name: 'detect_adapters', arguments: {} }];
        if (window.log) window.log('[Consola] El modelo no propuso herramientas; orquestador mínimo local (scan + adaptadores).', 'warn');
      }
      if (calls.length) {
        window._iaConsoleTools = true;
        var lines = [];
        var lastPcap = null, lastHash = null;
        try {
          for (var i = 0; i < Math.min(calls.length, 12); i++) {
            var tc = calls[i] || {};
            var name = String(tc.name || '');
            var args = (tc.arguments && typeof tc.arguments === 'object') ? tc.arguments : {};
            if (name === 'convert_pcap' && lastPcap) args.pcap_path = lastPcap;
            if (name === 'inspect_hash' && lastHash) args.hash_path = lastHash;
            if (name === 'crack_hash' && lastHash) args.hash_file = lastHash;
            var v = window.iaValidateTool(name, args);
            if (!v.ok) { lines.push('✗ ' + name + ': ' + v.error); continue; }
            var ok = await window.iaConfirm('Consola → ' + name + '\nArgumentos: ' + JSON.stringify(args));
            if (!ok) { lines.push('Cancelado por el usuario en ' + name); break; }
            try {
              var res = await window.iaExecuteTool(name, args);
              lines.push('✓ ' + name + ' → ' + JSON.stringify(res).slice(0, 300));
              var out = res && res.output_file;
              if (typeof out === 'string' && out) {
                if (/\.22000$/i.test(out)) lastHash = out;
                else if (/\.pcap(ng)?$/i.test(out)) lastPcap = out;
              }
            } catch (e) { lines.push('✗ ' + name + ': ' + e); }
          }
        } finally { window._iaConsoleTools = false; }
        return lines.join('\n') || 'Sin resultados.';
      }
      if (t && t.response) reply = String(t.response).trim();
      else if (t && t.error) reply = 'Error de la IA: ' + t.error;
    }
  } catch (e) {}
  if (!reply) {
    try {
      var r = await window.__invoke('ollama_chat', {
        prompt: 'Contexto: laboratorio WiFi, redes propias o autorizadas. Responde en español, conciso.\n\n' + text,
        model: null
      });
      if (r && r.success) reply = (r.response || '').trim();
      else if (r && r.error) reply = 'Error de la IA: ' + r.error;
    } catch (e) {}
  }
  if (!reply) reply = 'IA no disponible (Ollama en localhost:11434).';
  return reply;
};

window.iaConsoleAskCommand = async function (q) {
  var cmd = String(q || '').trim().toLowerCase();
  if (cmd === 'ayuda' || cmd === 'help') return 'Comandos: escanear | atacar | estado | diccionario | ayuda. O pide algo con herramientas (p. ej. «usa las tools para atacar»).';
  if (/^(recuerda|aprende|guarda)/.test(cmd)) {
    var val = String(q || '').replace(/^(recuerda|aprende|guarda)\s*/i, '').trim();
    if (!val) return 'Dime qué quieres que recuerde: «recuerda usar PMKID primero».';
    try {
      var r = await window.__invoke('ia_memory_add', { key: 'leccion', value: val });
      return (r && r.message) || 'Memoria actualizada.';
    } catch (e) { return 'No se pudo guardar en memoria: ' + e; }
  }
  if (/^(qué sabes|que sabes|memoria|notas)/.test(cmd)) {
    try {
      var ns = await window.__invoke('ia_memory_list');
      if (!ns || !ns.length) return 'Todavía no he aprendido nada. Usa «recuerda …».';
      return ns.map(function (n) { return '• ' + n.key + ': ' + n.value; }).join('\n');
    } catch (e) { return 'No se pudo leer la memoria: ' + e; }
  }
  if (/^(pmkid|provocar pmkid)/.test(cmd)) {
    var b = (window.getSelectedBssid && window.getSelectedBssid()) || (document.getElementById('attack-bssid') || {}).value || '';
    var s = (document.getElementById('simple-target-label')?.textContent || '').replace('Objetivo: ', '');
    if (!b || !s) return 'Selecciona primero una red objetivo.';
    try { var r = await window.iaPmkidProbe({ ssid: s, bssid: b, channel: null }); return (r && r.message) || JSON.stringify(r); }
    catch (e) { return 'pmkid_probe falló: ' + e; }
  }
  if (/^(gatillo|trigger|auto.?gatillo)/.test(cmd)) {
    var b = (window.getSelectedBssid && window.getSelectedBssid()) || (document.getElementById('attack-bssid') || {}).value || '';
    var s = (document.getElementById('simple-target-label')?.textContent || '').replace('Objetivo: ', '');
    if (!b || !s) return 'Selecciona primero una red objetivo en Escanear.';
    try {
      var t = await window.iaTriggerHandshake({ ssid: s, bssid: b, channel: null });
      return (t && t.message) || 'Auto-gatillo terminado.';
    } catch (e) { return 'Auto-gatillo falló: ' + e; }
  }
  if (/^(diccionario.?ia|wordlist.?ia|diccionario con ia|genera diccionario.?ia)/.test(cmd)) {
    var bssid = (window.getSelectedBssid && window.getSelectedBssid()) || (document.getElementById('attack-bssid') || {}).value || '';
    var label = (document.getElementById('simple-target-label')?.textContent || '').replace('Objetivo: ', '');
    try {
      var d = await window.iaExecuteTool('generate_dictionary_ai', { ssid: label, bssid: bssid, hints: q, limit: 400 });
      return (d && d.message) || ('Diccionario IA: ' + JSON.stringify(d));
    } catch (e) { return 'No se pudo crear el diccionario con IA: ' + e; }
  }
  if (/^(cazar todas|cazar todas las redes|caza todas|hunt all|buscar todas)/.test(cmd)) {
    try {
      if (typeof window.huntAllNetworks !== 'function') return 'Cazador masivo no disponible en esta versión.';
      const r = await window.huntAllNetworks();
      return '[cazador-masivo] ' + (r && r.total || 0) + ' redes · ' + (r && r.captured || 0) +
        ' con material · ' + (r && r.failed || 0) + ' sin material.';
    } catch (e) { return 'Cazador masivo falló: ' + e; }
  }
  if (/^(diccionario|wordlist|genera diccionario)/.test(cmd)) {
    var bssid = (window.getSelectedBssid && window.getSelectedBssid()) || (document.getElementById('attack-bssid') || {}).value || '';
    var label = (document.getElementById('simple-target-label')?.textContent || '').replace('Objetivo: ', '');
    try {
      var d = await window.iaExecuteTool('generate_dictionary', { ssid: label, bssid: bssid, hints: q });
      return (d && d.message) || ('Diccionario: ' + JSON.stringify(d));
    } catch (e) { return 'No se pudo crear el diccionario: ' + e; }
  }
  if (cmd === 'estado') {
    var busy = (typeof window.attackIsBusy === 'function') && window.attackIsBusy();
    return busy ? 'Hay un ataque en curso (lock activo).' : 'No hay ataque en curso (lock libre).';
  }
  if (cmd === 'escanear' || cmd === 'scan') {
    try { await window.doScan(); } catch (e) { return 'Fallo del escaneo: ' + e; }
    return 'Escaneo lanzado.';
  }
  // Intención de ataque en lenguaje natural: usar el pipeline IA validado
  // (E0-18), NO el chat y NO el ataque determinista.
  var attackIntent = /\b(atacar|ataca|atac|ataque|attack|crack|romper|hackear|hack|intrus)\b/i.test(cmd) ||
    /\b(usa|usar|utiliza|emplea)\b.{0,40}\b(tools|herramientas)\b/i.test(cmd);
  if (attackIntent) {
    try {
      if (window.setIaMode) window.setIaMode(true);
      if (window.iaAttack) await window.iaAttack();
      else await window.simpleAttack();
      return 'Modo IA lanzado desde la consola. Confirma cada paso en el panel y revisa los logs.';
    } catch (e) { return 'Fallo al lanzar el ataque: ' + e; }
  }
  return window.iaConsoleAsk(q);
};

// entrada por Enter
(function () {
  var cin = document.getElementById("console-input");
  if (!cin) return;
  cin.addEventListener("keydown", async function (e) {
    if (e.key !== "Enter") return;
    e.preventDefault();
    var q = (cin.value || "").trim();
    cin.value = "";
    if (!q) return;
    window.iaConsoleLog("> " + q, "user");
    window.setIaStatus("IA: procesando consulta...");
    var ans;
    try { ans = await window.iaConsoleAskCommand(q); }
    catch (e2) { ans = "Error: " + e2; }
    window.iaConsoleLog("IA: " + ans, "ia");
    window.setIaStatus("IA: lista para atacar.");
  });
})();
