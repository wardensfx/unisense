"use strict";

const invoke = (cmd, args) => window.__TAURI__.core.invoke(cmd, args);

/** Mirrors core/src/scaling.rs::compute_factor exactly, for live preview. */
function computeFactor(targetCm360, dpi, constant, refSens, curSens) {
  if (!(targetCm360 > 0) || !(dpi > 0) || !(constant > 0) || !(refSens > 0) || !(curSens > 0)) {
    return 1;
  }
  const sensRatio = curSens / refSens;
  const denom = targetCm360 * dpi * constant * sensRatio;
  if (!(denom > 0) || !isFinite(denom)) return 1;
  const f = (2.54 * 360) / denom;
  return isFinite(f) && f > 0 ? f : 1;
}

const defaultSettings = () => ({
  mouse_dpi: 800,
  target_cm_per_360: 35.0,
  toggle_hotkey: "Mouse4",
  cycle_game_hotkey: "Ctrl+Alt+F9",
  start_in_passthrough: true,
});

const state = {
  settings: defaultSettings(),
  games: [],
  editingIndex: null, // null = nouveau jeu
  processes: [],
  snapA: null,
  snapB: null,
};

// ---------------------------------------------------------------- helpers

const $ = (id) => document.getElementById(id);

function showStatus(message, isError = false) {
  const el = $("status");
  el.textContent = message;
  el.classList.toggle("error", isError);
  if (!isError) {
    clearTimeout(showStatus._t);
    showStatus._t = setTimeout(() => (el.textContent = ""), 3500);
  }
}

function showBanner(message) {
  const el = $("banner");
  el.textContent = message;
  el.hidden = !message;
}

function fmtFactor(f) {
  return `×${f.toFixed(3).replace(/0+$/, "").replace(/\.$/, ".0")}`;
}

// ---------------------------------------------------------------- gauge

const GAUGE_R = 82;
const GAUGE_C = 2 * Math.PI * GAUGE_R;
const GAUGE_MIN_CM = 10;
const GAUGE_MAX_CM = 60;

function initGaugeTicks() {
  const g = $("gauge-ticks");
  const cx = 100, cy = 100, rOuter = 96, rInner = 88;
  let html = "";
  for (let i = 0; i < 24; i++) {
    const a = (i / 24) * Math.PI * 2;
    const x1 = cx + Math.cos(a) * rInner;
    const y1 = cy + Math.sin(a) * rInner;
    const x2 = cx + Math.cos(a) * rOuter;
    const y2 = cy + Math.sin(a) * rOuter;
    html += `<line x1="${x1.toFixed(1)}" y1="${y1.toFixed(1)}" x2="${x2.toFixed(1)}" y2="${y2.toFixed(1)}" />`;
  }
  g.innerHTML = html;
}

function updateGauge() {
  const sweep = $("gauge-sweep");
  sweep.style.strokeDasharray = `${GAUGE_C.toFixed(2)}`;
  const target = Number(state.settings.target_cm_per_360) || 0;
  const frac = Math.min(1, Math.max(0, (target - GAUGE_MIN_CM) / (GAUGE_MAX_CM - GAUGE_MIN_CM)));
  sweep.style.strokeDashoffset = `${(GAUGE_C * (1 - frac)).toFixed(2)}`;
  $("cm360-readout").textContent = target > 0 ? target.toFixed(1) : "—";
}

// ---------------------------------------------------------------- render

function renderSettingsFields() {
  $("target-cm360").value = state.settings.target_cm_per_360;
  $("mouse-dpi").value = state.settings.mouse_dpi;
  $("toggle-hotkey").value = state.settings.toggle_hotkey;
  $("cycle-hotkey").value = state.settings.cycle_game_hotkey;
  $("start-passthrough").checked = !!state.settings.start_in_passthrough;
  updateGauge();
}

function renderGames() {
  const grid = $("games-grid");
  const empty = $("empty-state");
  $("games-count").textContent =
    state.games.length === 0 ? "" : `${state.games.length} jeu${state.games.length > 1 ? "x" : ""}`;

  if (state.games.length === 0) {
    grid.hidden = true;
    empty.hidden = false;
    return;
  }
  grid.hidden = false;
  empty.hidden = true;

  const dpi = Number(state.settings.mouse_dpi) || 0;
  const target = Number(state.settings.target_cm_per_360) || 0;

  grid.innerHTML = "";
  state.games.forEach((game, i) => {
    const factor = computeFactor(
      target,
      dpi,
      game.deg_per_count_at_ref,
      game.reference_sensitivity,
      game.current_sensitivity ?? game.reference_sensitivity
    );
    const card = document.createElement("div");
    card.className = "game-card";
    card.tabIndex = 0;
    card.innerHTML = `
      <div class="game-card-top">
        <h3>${escapeHtml(game.name || "(sans nom)")}</h3>
        <span class="factor-badge">${fmtFactor(factor)}</span>
      </div>
      <dl>
        <dt>constante</dt><dd>${game.deg_per_count_at_ref}</dd>
        <dt>sensi réf.</dt><dd>${game.reference_sensitivity}</dd>
      </dl>
      <div class="game-card-tags">
        ${game.hotkey ? `<span class="tag">⌨ ${escapeHtml(game.hotkey)}</span>` : ""}
        ${game.auto_detect ? `<span class="tag">◎ auto</span>` : ""}
      </div>
    `;
    card.addEventListener("click", () => openEditor(i));
    card.addEventListener("keydown", (e) => {
      if (e.key === "Enter") openEditor(i);
    });
    grid.appendChild(card);
  });

  const addCard = document.createElement("div");
  addCard.className = "game-card add-card";
  addCard.tabIndex = 0;
  addCard.innerHTML = `<span class="plus">+</span><span>Ajouter un jeu</span>`;
  addCard.addEventListener("click", () => openEditor(null));
  grid.appendChild(addCard);
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
  }[c]));
}

// ---------------------------------------------------------------- drawer

function openEditor(index) {
  state.editingIndex = index;
  const isNew = index === null;
  const game = isNew
    ? { name: "", deg_per_count_at_ref: "", reference_sensitivity: 1, current_sensitivity: null, hotkey: "", source: "", auto_detect: null }
    : state.games[index];

  $("drawer-title").textContent = isNew ? "Ajouter un jeu" : "Modifier le jeu";
  $("game-name").value = game.name || "";
  $("game-constant").value = game.deg_per_count_at_ref ?? "";
  $("game-ref-sens").value = game.reference_sensitivity ?? "";
  $("game-current-sens").value = game.current_sensitivity ?? "";
  $("game-hotkey").value = game.hotkey || "";
  $("game-source").value = game.source || "";
  $("delete-game-btn").hidden = isNew;

  const ad = game.auto_detect;
  $("auto-detect-enabled").checked = !!ad;
  $("auto-detect-fields").hidden = !ad;
  $("offset-hex").value = ad ? `0x${ad.offset.toString(16).toUpperCase()}` : "";
  $("in-game-bytes").value = ad ? ad.in_game_bytes.map((b) => b.toString(16).padStart(2, "0")).join(" ") : "";
  $("poll-interval").value = ad ? ad.poll_interval_ms : 250;
  state.pendingModuleName = ad?.module_name || null;
  state.pendingProcessName = ad?.process_name || null;
  $("assist-details").open = false;
  resetScanUI();

  updateGamePreview();

  $("drawer-backdrop").hidden = false;
  $("drawer").hidden = false;
  $("game-name").focus();

  if (ad && state.processes.length === 0) refreshProcesses();
}

function closeEditor() {
  $("drawer-backdrop").hidden = true;
  $("drawer").hidden = true;
  state.editingIndex = null;
}

function updateGamePreview() {
  const constant = parseFloat($("game-constant").value);
  const refSens = parseFloat($("game-ref-sens").value);
  const curSens = parseFloat($("game-current-sens").value) || refSens;
  const dpi = Number(state.settings.mouse_dpi) || 0;
  const target = Number(state.settings.target_cm_per_360) || 0;
  const el = $("game-preview");

  if (!(constant > 0) || !(refSens > 0)) {
    el.innerHTML = `Renseignez la constante et la sensi de référence pour voir le facteur.`;
    return;
  }
  const factor = computeFactor(target, dpi, constant, refSens, curSens);
  el.innerHTML = `Facteur appliqué : <strong>${fmtFactor(factor)}</strong> à ${dpi || "?"} DPI pour ${target || "?"} cm/360°`;
}

function buildGameFromForm() {
  const name = $("game-name").value.trim();
  const constant = parseFloat($("game-constant").value);
  const refSens = parseFloat($("game-ref-sens").value);
  const curSensRaw = $("game-current-sens").value.trim();

  if (!name) throw new Error("Le nom du jeu est requis.");
  if (!(constant > 0)) throw new Error("La constante doit être un nombre positif.");
  if (!(refSens > 0)) throw new Error("La sensi de référence doit être un nombre positif.");

  const game = {
    name,
    deg_per_count_at_ref: constant,
    reference_sensitivity: refSens,
    current_sensitivity: curSensRaw ? parseFloat(curSensRaw) : null,
    hotkey: $("game-hotkey").value.trim() || null,
    source: $("game-source").value.trim() || null,
    auto_detect: null,
  };

  if ($("auto-detect-enabled").checked) {
    const processName = state.pendingProcessName;
    const offsetRaw = $("offset-hex").value.trim();
    const bytesRaw = $("in-game-bytes").value.trim();
    if (!processName) throw new Error("Sélectionnez un processus pour la détection automatique.");
    if (!/^0x[0-9a-f]+$/i.test(offsetRaw)) throw new Error("Offset invalide (format attendu : 0xABCDEF).");
    const bytes = bytesRaw
      .split(/\s+/)
      .filter(Boolean)
      .map((h) => parseInt(h, 16));
    if (bytes.length === 0 || bytes.some((b) => Number.isNaN(b) || b < 0 || b > 255)) {
      throw new Error("Octets « en jeu » invalides (ex : 01 ou 01 00).");
    }
    game.auto_detect = {
      process_name: processName,
      module_name: state.pendingModuleName || null,
      offset: parseInt(offsetRaw, 16),
      pointer_chain: [],
      in_game_bytes: bytes,
      poll_interval_ms: parseInt($("poll-interval").value, 10) || 250,
    };
  }

  return game;
}

function applyGame() {
  try {
    const game = buildGameFromForm();
    if (state.editingIndex === null) {
      state.games.push(game);
    } else {
      state.games[state.editingIndex] = game;
    }
    renderGames();
    closeEditor();
    showBanner("");
  } catch (e) {
    showBanner(e.message || String(e));
  }
}

function deleteGame() {
  if (state.editingIndex === null) return;
  state.games.splice(state.editingIndex, 1);
  renderGames();
  closeEditor();
}

// ---------------------------------------------------------------- assist (process/module + memory diff)

async function refreshProcesses() {
  try {
    state.processes = await invoke("list_processes");
    const sel = $("process-select");
    sel.innerHTML = `<option value="">— choisir —</option>` + state.processes
      .map((p) => `<option value="${p.pid}">${escapeHtml(p.name)} (${p.pid})</option>`)
      .join("");
    if (state.pendingProcessName) {
      const match = state.processes.find((p) => p.name.toLowerCase() === state.pendingProcessName.toLowerCase());
      if (match) {
        sel.value = String(match.pid);
        await refreshModules();
      }
    }
  } catch (e) {
    setScanStatus(`Erreur liste des process : ${e}`);
  }
}

async function refreshModules() {
  const pid = Number($("process-select").value);
  const sel = $("module-select");
  sel.innerHTML = `<option value="">(module principal)</option>`;
  if (!pid) return;
  const selectedProc = state.processes.find((p) => p.pid === pid);
  state.pendingProcessName = selectedProc ? selectedProc.name : null;
  try {
    const modules = await invoke("list_modules", { pid });
    sel.innerHTML += modules.map((m) => `<option value="${escapeHtml(m)}">${escapeHtml(m)}</option>`).join("");
    if (state.pendingModuleName) sel.value = state.pendingModuleName;
  } catch (e) {
    setScanStatus(`Erreur liste des modules : ${e}`);
  }
  resetScanUI();
}

function setScanStatus(msg) {
  $("scan-status").textContent = msg;
}

function resetScanUI() {
  state.snapA = null;
  state.snapB = null;
  $("diff-btn").disabled = true;
  $("scan-results").hidden = true;
  $("scan-results-body").innerHTML = "";
  setScanStatus("");
}

async function takeSnapshot(which) {
  const pid = Number($("process-select").value);
  if (!pid) {
    setScanStatus("Choisissez un processus d'abord.");
    return;
  }
  setScanStatus(`Capture ${which}…`);
  try {
    const id = await invoke("mem_snapshot", { pid });
    if (which === "A") state.snapA = id;
    else state.snapB = id;
    setScanStatus(
      `État A : ${state.snapA ? "capturé" : "—"}   ·   État B : ${state.snapB ? "capturé" : "—"}`
    );
    $("diff-btn").disabled = !(state.snapA && state.snapB);
  } catch (e) {
    setScanStatus(`Erreur de capture : ${e}`);
  }
}

async function runDiff() {
  setScanStatus("Comparaison…");
  try {
    const results = await invoke("mem_diff", { snapshotA: state.snapA, snapshotB: state.snapB });
    const body = $("scan-results-body");
    if (results.length === 0) {
      setScanStatus("Aucune différence trouvée dans la zone scannée. Changez d'état plus franchement et refaites l'essai.");
      $("scan-results").hidden = true;
      return;
    }
    setScanStatus(`${results.length} octet(s) modifié(s) (limité aux ${results.length >= 500 ? "500 premiers" : results.length}).`);
    body.innerHTML = results
      .map((r, idx) => {
        const canUse = !!r.module;
        return `
        <tr>
          <td>${r.address_hex}</td>
          <td>${r.module ? escapeHtml(r.module) + (r.offset_hex ? " + " + r.offset_hex : "") : "hors module"}</td>
          <td>0x${r.value_a.toString(16).padStart(2, "0")}</td>
          <td class="value-changed">0x${r.value_b.toString(16).padStart(2, "0")}</td>
          <td>${canUse ? `<button class="use-row-btn" data-idx="${idx}">Utiliser</button>` : "—"}</td>
        </tr>`;
      })
      .join("");
    $("scan-results").hidden = false;
    body.querySelectorAll(".use-row-btn").forEach((btn) => {
      btn.addEventListener("click", () => {
        const r = results[Number(btn.dataset.idx)];
        state.pendingModuleName = r.module;
        $("module-select").value = r.module;
        $("offset-hex").value = r.offset_hex;
        $("in-game-bytes").value = r.value_b.toString(16).padStart(2, "0");
        setScanStatus(`Candidat appliqué : ${r.module} + ${r.offset_hex} = 0x${r.value_b.toString(16)} en jeu.`);
      });
    });
  } catch (e) {
    setScanStatus(`Erreur de comparaison : ${e}`);
  }
}

// ---------------------------------------------------------------- config load/save

async function loadConfig() {
  try {
    const cfg = await invoke("load_config");
    state.settings = cfg.settings;
    state.games = cfg.games;
    renderSettingsFields();
    renderGames();
    showBanner("");
  } catch (e) {
    const msg = String(e);
    if (msg.includes("lecture impossible")) {
      // Pas encore de config a cet emplacement : etat de premier lancement, pas une erreur.
      state.settings = defaultSettings();
      state.games = [];
      renderSettingsFields();
      renderGames();
    } else {
      showBanner(`Config illisible : ${msg}`);
    }
  }
}

async function loadExample() {
  try {
    const cfg = await invoke("load_example_config");
    state.settings = cfg.settings;
    state.games = cfg.games;
    renderSettingsFields();
    renderGames();
    showBanner("");
    showStatus("Config d'exemple chargée — pensez à Enregistrer.");
  } catch (e) {
    showBanner(`Impossible de charger l'exemple : ${e}`);
  }
}

async function saveConfig() {
  state.settings.target_cm_per_360 = parseFloat($("target-cm360").value) || 0;
  state.settings.mouse_dpi = parseFloat($("mouse-dpi").value) || 0;
  state.settings.toggle_hotkey = $("toggle-hotkey").value.trim();
  state.settings.cycle_game_hotkey = $("cycle-hotkey").value.trim();
  state.settings.start_in_passthrough = $("start-passthrough").checked;

  try {
    await invoke("save_config", { config: { settings: state.settings, games: state.games } });
    showStatus("Enregistré ✓");
  } catch (e) {
    showStatus(`Échec de l'enregistrement : ${e}`, true);
  }
}

// ---------------------------------------------------------------- wiring

function wireEvents() {
  $("target-cm360").addEventListener("input", () => {
    state.settings.target_cm_per_360 = parseFloat($("target-cm360").value) || 0;
    updateGauge();
    renderGames();
  });
  $("mouse-dpi").addEventListener("input", () => {
    state.settings.mouse_dpi = parseFloat($("mouse-dpi").value) || 0;
    renderGames();
  });
  $("save-btn").addEventListener("click", saveConfig);
  $("load-example-btn").addEventListener("click", loadExample);
  $("empty-load-example-btn").addEventListener("click", loadExample);
  $("add-game-btn").addEventListener("click", () => openEditor(null));
  $("empty-add-game-btn").addEventListener("click", () => openEditor(null));

  $("drawer-close").addEventListener("click", closeEditor);
  $("drawer-backdrop").addEventListener("click", closeEditor);
  $("apply-game-btn").addEventListener("click", applyGame);
  $("delete-game-btn").addEventListener("click", deleteGame);
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && !$("drawer").hidden) closeEditor();
  });

  ["game-constant", "game-ref-sens", "game-current-sens"].forEach((id) =>
    $(id).addEventListener("input", updateGamePreview)
  );

  $("auto-detect-enabled").addEventListener("change", (e) => {
    $("auto-detect-fields").hidden = !e.target.checked;
    if (e.target.checked && state.processes.length === 0) refreshProcesses();
  });
  $("refresh-processes-btn").addEventListener("click", refreshProcesses);
  $("process-select").addEventListener("change", refreshModules);
  $("module-select").addEventListener("change", (e) => {
    state.pendingModuleName = e.target.value || null;
  });
  $("snapshot-a-btn").addEventListener("click", () => takeSnapshot("A"));
  $("snapshot-b-btn").addEventListener("click", () => takeSnapshot("B"));
  $("diff-btn").addEventListener("click", runDiff);
}

async function init() {
  initGaugeTicks();
  wireEvents();
  renderSettingsFields();
  try {
    $("config-path").textContent = await invoke("config_path_string");
    $("config-path").title = $("config-path").textContent;
  } catch {
    /* purement cosmetique, on ignore */
  }
  await loadConfig();
}

init();
