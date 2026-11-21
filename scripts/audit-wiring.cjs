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

if (bad) { console.error('audit: ' + bad + ' issue(s)'); process.exit(1); }
console.log('audit-wiring: OK (' + invokes.size + ' invokes, ' + onclicks.size + ' onclicks)');
