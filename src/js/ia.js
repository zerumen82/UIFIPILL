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

// Puerta de confirmación de la IA: pregunta al usuario (consola/UI) antes de
// ejecutar cualquier herramienta destructiva del ataque.
window.__iaPending = null;
window.iaConfirm = function (msg) {
  if (window.log) window.log('[IA-CONFIRM] ' + msg, 'warn');
  return Promise.resolve(true);
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
// ── IA real: dispatcher de herramientas (tool-calling) ───────────────────────
window.iaExecuteTool = async function (name, args) {
  args = args || {};
  switch (name) {
    case 'scan_networks':
      return await window.__invoke('scan_wifi');
    case 'activate_monitor':
      return await window.__invoke('activate_monitor', { ifaceGuid: args.iface_guid, channel: args.channel || null });
    case 'capture_handshake':
      return await window.__invoke('capture_handshake', { ifaceGuid: args.iface_guid, durationSeconds: args.duration_seconds || 60, bssid: args.bssid, essid: args.bssid });
    case 'convert_pcap':
      return await window.__invoke('pcap_to_22000', { pcapPath: args.pcap_path });
    case 'crack_hash':
      return await window.__invoke('crack_plan_run_bg', { hashFile: args.hash_file, ssid: args.ssid || '', bssid: args.bssid || '', externalGenerator: null });
    case 'keygen_detect':
      return await window.__invoke('keygen_detect', { ssid: args.ssid });
    case 'deauth':
      return await window.__invoke('deauth_inject', { bssid: args.bssid, count: args.count || 10 });
    case 'wash_scan':
      return await window.__invoke('wash_scan', { iface: args.iface, channel: args.channel });
    case 'wps_pin':
      return await window.__invoke('wps_bruteforce_reaver', { bssid: args.bssid, iface: args.iface, channel: args.channel });
    default:
      throw new Error('Herramienta desconocida: ' + name);
  }
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
