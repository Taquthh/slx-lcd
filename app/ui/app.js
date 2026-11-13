const invoke = window.__TAURI__.core.invoke;
const $ = (id) => document.getElementById(id);
const CANVAS_SIZE = 480;
const MEDIA_FPS = 2;
const MAX_MEDIA_FRAMES = 12;

let theme = null;
let currentThemeKey = "aqua";
let selectedWidget = -1;
let daemonActive = false;
let previewBusy = false;
let previewQueued = false;
let dragState = null;
let previewTimer = null;
let statsTimer = null;
let toastTimer = null;
let renderTimer = null;
let fieldCounter = 0;

function toast(message, error = false) {
  const el = $("toast");
  el.textContent = message;
  el.className = `toast visible${error ? " error" : ""}`;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { el.className = "toast"; }, 3200);
}

function slug(value) {
  return String(value || "")
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 64);
}

function dims() {
  return {
    width: Math.max(1, Number(theme?.width) || CANVAS_SIZE),
    height: Math.max(1, Number(theme?.height) || CANVAS_SIZE),
  };
}

function serializableTheme() {
  return JSON.stringify(theme, null, 2);
}

function updateSource() {
  if (!$("json-panel").hidden) $("json-source").value = serializableTheme();
}

function queuePreview(force = true) {
  clearTimeout(renderTimer);
  renderTimer = setTimeout(() => renderPreview(force), 180);
}

async function renderPreview(force = false) {
  if (!theme || (!daemonActive && !force)) return;
  if (previewBusy) {
    previewQueued = previewQueued || force;
    return;
  }
  previewBusy = true;
  try {
    const image = await invoke("preview", { themeJson: serializableTheme() });
    $("preview").src = image;
  } catch (error) {
    toast(String(error), true);
  } finally {
    previewBusy = false;
    if (previewQueued) {
      previewQueued = false;
      renderPreview(true);
    }
  }
}

function widgetName(widget) {
  const labels = { text: "Teks", rect: "Bentuk", bar: "Progress", gauge: "Gauge", clock: "Jam", image: "Media" };
  if (widget.type === "text") return widget.text || "Teks";
  if (widget.type === "image") return widget.frames?.length > 1 ? "Animasi media" : "Gambar";
  return labels[widget.type] || widget.type;
}

function widgetBounds(widget) {
  if (widget.type === "gauge") {
    return { x: widget.cx - widget.r, y: widget.cy - widget.r, w: widget.r * 2, h: widget.r * 2 };
  }
  if (widget.type === "text" || widget.type === "clock") {
    const text = widget.type === "text" ? widget.text || "Teks" : "00:00";
    const size = Number(widget.size) || 24;
    return { x: widget.x, y: widget.y, w: Math.max(40, text.length * size * 0.62), h: Math.max(24, size * 1.35) };
  }
  return { x: widget.x, y: widget.y, w: Number(widget.w) || 100, h: Number(widget.h) || 60 };
}

function renderOverlay() {
  const layer = $("widget-layer");
  const { width, height } = dims();
  layer.replaceChildren(...(theme?.widgets || []).map((widget, index) => {
    const box = widgetBounds(widget);
    const button = document.createElement("button");
    button.type = "button";
    button.className = `canvas-widget${selectedWidget === index ? " selected" : ""}`;
    button.dataset.index = String(index);
    button.setAttribute("aria-label", `Pilih ${widgetName(widget)}`);
    button.setAttribute("aria-pressed", String(selectedWidget === index));
    button.style.left = `${box.x / width * 100}%`;
    button.style.top = `${box.y / height * 100}%`;
    button.style.width = `${Math.max(8, box.w / width * 100)}%`;
    button.style.height = `${Math.max(8, box.h / height * 100)}%`;
    const tag = document.createElement("span");
    tag.className = "widget-tag";
    tag.textContent = widgetName(widget);
    button.append(tag);
    button.addEventListener("pointerdown", (event) => beginWidgetDrag(event, index));
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      selectedWidget = index;
      renderSelection();
    });
    return button;
  }));
  $("empty-state").hidden = Boolean(theme?.widgets?.length);
}

function renderLayers() {
  const list = $("layers");
  const widgets = theme?.widgets || [];
  $("layer-count").textContent = String(widgets.length);
  if (!widgets.length) {
    list.innerHTML = '<div class="layer-empty">Belum ada komponen. Tambahkan dari panel ini.</div>';
    return;
  }
  list.replaceChildren(...widgets.map((widget, index) => {
    const row = document.createElement("button");
    row.type = "button";
    row.draggable = true;
    row.className = `layer-item${selectedWidget === index ? " selected" : ""}`;
    row.dataset.index = String(index);
    const kind = document.createElement("span");
    kind.className = "layer-kind";
    kind.textContent = widget.type.slice(0, 1);
    const name = document.createElement("span");
    name.className = "layer-name";
    name.textContent = widgetName(widget);
    row.append(kind, name);
    row.addEventListener("click", () => { selectedWidget = index; renderSelection(); });
    row.addEventListener("dragstart", (event) => {
      event.dataTransfer.setData("application/x-slx-layer", String(index));
      event.dataTransfer.effectAllowed = "move";
    });
    row.addEventListener("dragover", (event) => event.preventDefault());
    row.addEventListener("drop", (event) => {
      event.preventDefault();
      const from = Number(event.dataTransfer.getData("application/x-slx-layer"));
      if (Number.isInteger(from) && from !== index && theme.widgets[from]) {
        const [item] = theme.widgets.splice(from, 1);
        theme.widgets.splice(index, 0, item);
        selectedWidget = index;
        updateModel();
      }
    });
    return row;
  }));
}

function field(label, value, onChange, options = {}) {
  const wrap = document.createElement("div");
  wrap.className = options.wide ? "field field-wide" : "field";
  const labelEl = document.createElement("label");
  labelEl.textContent = label;
  const input = document.createElement(options.multiline ? "textarea" : "input");
  input.id = `theme-property-${++fieldCounter}`;
  labelEl.htmlFor = input.id;
  if (!options.multiline) input.type = options.type || "text";
  if (options.type === "number") {
    input.step = options.step || "1";
    if (options.min != null) input.min = String(options.min);
    if (options.max != null) input.max = String(options.max);
  }
  if (options.multiline) input.rows = 3;
  if (options.type === "checkbox") input.checked = Boolean(value);
  else input.value = value ?? "";
  if (options.readonly) input.readOnly = true;
  input.addEventListener("input", () => onChange(options.type === "number" ? (input.value === "" ? null : Number(input.value)) : options.type === "checkbox" ? input.checked : input.value));
  wrap.append(labelEl, input);
  return wrap;
}

function choiceField(label, value, choices, onChange) {
  const wrap = document.createElement("div");
  wrap.className = "field";
  const labelEl = document.createElement("label");
  labelEl.textContent = label;
  const select = document.createElement("select");
  select.id = `theme-property-${++fieldCounter}`;
  labelEl.htmlFor = select.id;
  for (const choice of choices) {
    const option = document.createElement("option");
    option.value = choice;
    option.textContent = choice;
    select.append(option);
  }
  select.value = choices.includes(value) ? value : choices[0];
  select.addEventListener("change", () => onChange(select.value));
  wrap.append(labelEl, select);
  return wrap;
}

function colorField(label, value, onChange) {
  const wrap = document.createElement("div");
  wrap.className = "field";
  const labelEl = document.createElement("label");
  labelEl.textContent = label;
  const input = document.createElement("input");
  input.id = `theme-property-${++fieldCounter}`;
  labelEl.htmlFor = input.id;
  input.type = "color";
  input.value = /^#[0-9a-f]{6}$/i.test(value || "") ? value : "#25e0cc";
  input.addEventListener("input", () => onChange(input.value));
  wrap.append(labelEl, input);
  return wrap;
}

function renderInspector() {
  const root = $("inspector");
  root.replaceChildren();
  const heading = document.createElement("div");
  heading.className = "inspector-title";
  const title = document.createElement("div");
  const selected = theme?.widgets?.[selectedWidget];
  title.innerHTML = `<span class="eyebrow">${selected ? "SELECTED LAYER" : "THEME SETTINGS"}</span><h2>${selected ? escapeHtml(widgetName(selected)) : "Tema & kanvas"}</h2>`;
  heading.append(title);
  if (selected) {
    const actions = document.createElement("div");
    actions.className = "inspector-actions";
    const remove = document.createElement("button");
    remove.type = "button";
    remove.textContent = "×";
    remove.title = "Hapus layer";
    remove.setAttribute("aria-label", "Hapus layer terpilih");
    remove.onclick = () => {
      theme.widgets.splice(selectedWidget, 1);
      selectedWidget = -1;
      updateModel();
    };
    actions.append(remove);
    heading.append(actions);
  }
  root.append(heading);
  const fields = document.createElement("div");
  fields.className = "field-list";

  if (!selected) {
    fields.append(field("Nama tema", theme.name || "", (value) => { theme.name = value; $("theme-name").value = value; updateSource(); }));
    const dimensions = document.createElement("div");
    dimensions.className = "field-row";
    dimensions.append(
      field("Lebar", theme.width, (value) => { if (value) theme.width = Math.max(64, Math.min(480, value)); updateModel(false); }, { type: "number", min: 64, max: 480 }),
      field("Tinggi", theme.height, (value) => { if (value) theme.height = Math.max(64, Math.min(480, value)); updateModel(false); }, { type: "number", min: 64, max: 480 })
    );
    fields.append(dimensions);
    const help = document.createElement("p");
    help.className = "field-note";
    help.textContent = "Layar perangkat berukuran 480 × 480. Ubah ukuran kanvas untuk desain khusus.";
    fields.append(help);
  } else {
    const keys = Object.keys(selected).filter((key) => !["type", "frames", "path"].includes(key));
    const numeric = new Set(["x", "y", "w", "h", "cx", "cy", "r", "size", "radius", "min", "max", "thickness", "start", "sweep", "fps"]);
    const colors = new Set(["color", "fg", "bg"]);
    const onChange = (key, value) => {
      if (value === null) delete selected[key];
      else selected[key] = value;
      renderLayers();
      renderOverlay();
      updateSource();
      queuePreview(true);
    };
    for (const key of keys) {
      const label = key.replaceAll("_", " ").replace(/\b\w/g, (letter) => letter.toUpperCase());
      if (colors.has(key)) fields.append(colorField(label, selected[key], (value) => onChange(key, value)));
      else if (key === "bold") fields.append(field(label, selected[key], (value) => onChange(key, value), { type: "checkbox" }));
      else if (key === "align") fields.append(choiceField(label, selected[key], ["left", "center", "right"], (value) => onChange(key, value)));
      else if (numeric.has(key)) fields.append(field(label, selected[key], (value) => onChange(key, value), { type: "number", min: key === "fps" ? 0.2 : undefined, max: key === "fps" ? 10 : undefined, step: key === "fps" ? "0.2" : "1" }));
      else fields.append(field(label, selected[key], (value) => onChange(key, value), { multiline: key === "text" }));
    }
    if (selected.type === "image") {
      const mediaInfo = document.createElement("p");
      mediaInfo.className = "field-note";
      mediaInfo.textContent = selected.frames?.length > 1 ? `${selected.frames.length} frame · loop ${Number(selected.fps) || MEDIA_FPS} fps` : "Gambar statis";
      fields.append(mediaInfo);
      const replace = document.createElement("button");
      replace.type = "button";
      replace.className = "quiet-button";
      replace.textContent = "Tambah media";
      replace.onclick = () => $("media-input").click();
      fields.append(replace);
    }
  }
  root.append(fields);
  const tip = document.createElement("p");
  tip.className = "field-note";
  tip.textContent = selected ? "Seret layer di kanvas untuk memosisikannya. Edit nilai sensor melalui JSON, misalnya cpu.usage." : "Tambahkan teks, indikator, bentuk, atau media. JSON dapat dibuka untuk kontrol lanjutan.";
  root.append(tip);
}

function escapeHtml(value) {
  return String(value).replace(/[&<>"']/g, (char) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[char]);
}

function renderSelection() {
  renderLayers();
  renderOverlay();
  renderInspector();
  $("selection-label").textContent = selectedWidget >= 0 && theme.widgets[selectedWidget] ? widgetName(theme.widgets[selectedWidget]) : "Belum ada layer dipilih";
}

function updateModel(renderInspectorToo = true) {
  renderLayers();
  renderOverlay();
  if (renderInspectorToo) renderInspector();
  updateSource();
  queuePreview(true);
}

function defaultWidget(type, x, y) {
  const common = { text: { type: "text", x, y, text: "Teks baru", size: 30, color: "#edf3fa", bold: true, align: "left" },
    rect: { type: "rect", x, y, w: 120, h: 78, radius: 12, color: "#25e0cc" },
    bar: { type: "bar", x, y, w: 190, h: 15, value: "cpu.usage", min: 0, max: 100, fg: "#25e0cc", bg: "#243246", radius: 7 },
    gauge: { type: "gauge", cx: x + 72, cy: y + 72, r: 62, value: "cpu.usage", min: 0, max: 100, fg: "#9775ff", bg: "#243246", thickness: 11, start: 135, sweep: 270 },
    clock: { type: "clock", x, y, format: "%H:%M", size: 48, color: "#edf3fa", bold: true, align: "left" } };
  return structuredClone(common[type]);
}

function addWidget(type, x, y) {
  const { width, height } = dims();
  const widget = defaultWidget(type, Math.max(0, Math.min(width - 16, x)), Math.max(0, Math.min(height - 16, y)));
  theme.widgets.push(widget);
  selectedWidget = theme.widgets.length - 1;
  updateModel();
}

function stagePoint(event) {
  const rect = $("stage").getBoundingClientRect();
  const { width, height } = dims();
  return { x: (event.clientX - rect.left) / rect.width * width, y: (event.clientY - rect.top) / rect.height * height };
}

function beginWidgetDrag(event, index) {
  if (event.button !== 0) return;
  event.preventDefault();
  event.stopPropagation();
  selectedWidget = index;
  const widget = theme.widgets[index];
  const point = stagePoint(event);
  dragState = { index, startX: point.x, startY: point.y, startWidgetX: widget.x, startWidgetY: widget.y, startCx: widget.cx, startCy: widget.cy };
  renderSelection();
}

document.addEventListener("pointermove", (event) => {
  if (!dragState) return;
  const widget = theme.widgets[dragState.index];
  if (!widget) return;
  const point = stagePoint(event);
  const dx = point.x - dragState.startX;
  const dy = point.y - dragState.startY;
  if (widget.type === "gauge") {
    widget.cx = Math.max(0, Math.min(dims().width, dragState.startCx + dx));
    widget.cy = Math.max(0, Math.min(dims().height, dragState.startCy + dy));
  } else {
    widget.x = Math.max(0, Math.min(dims().width, dragState.startWidgetX + dx));
    widget.y = Math.max(0, Math.min(dims().height, dragState.startWidgetY + dy));
  }
  renderOverlay();
  updateSource();
  queuePreview(true);
});
document.addEventListener("pointerup", () => { if (dragState) { dragState = null; renderSelection(); } });

$("tool-palette").addEventListener("click", (event) => {
  const button = event.target.closest("[data-tool]");
  if (button) addWidget(button.dataset.tool, 125, 160);
});
$("tool-palette").addEventListener("dragstart", (event) => {
  const button = event.target.closest("[data-tool]");
  if (!button) return;
  event.dataTransfer.setData("application/x-slx-widget", button.dataset.tool);
  event.dataTransfer.effectAllowed = "copy";
});

$("stage").addEventListener("click", (event) => {
  if (event.target === $("stage") || event.target === $("preview") || event.target === $("widget-layer")) {
    selectedWidget = -1;
    renderSelection();
  }
});
$("stage").addEventListener("dragover", (event) => { event.preventDefault(); $("stage").classList.add("drop-target"); });
$("stage").addEventListener("dragleave", (event) => { if (!$("stage").contains(event.relatedTarget)) $("stage").classList.remove("drop-target"); });
$("stage").addEventListener("drop", async (event) => {
  event.preventDefault();
  $("stage").classList.remove("drop-target");
  const point = stagePoint(event);
  const file = event.dataTransfer.files[0];
  if (file) return addMedia(file, point.x, point.y);
  const type = event.dataTransfer.getData("application/x-slx-widget");
  if (type) addWidget(type, point.x, point.y);
});

function renderAll() {
  $("theme-name").value = theme.name || currentThemeKey;
  $("background-color").value = theme.background?.color || "#0b101e";
  renderLayers();
  renderOverlay();
  renderInspector();
  updateSource();
  queuePreview(true);
}

function normalizeTheme(value) {
  if (!value || typeof value !== "object" || !Array.isArray(value.widgets)) throw new Error("JSON tema harus memiliki objek dan daftar widgets.");
  value.name = String(value.name || "Tema baru");
  value.width = Math.max(64, Math.min(480, Number(value.width) || CANVAS_SIZE));
  value.height = Math.max(64, Math.min(480, Number(value.height) || CANVAS_SIZE));
  value.background = value.background || { color: "#0b101e" };
  return value;
}

async function refreshThemeList(select) {
  const names = await invoke("list_themes");
  const selectEl = $("theme-select");
  selectEl.replaceChildren(...names.map((name) => {
    const option = document.createElement("option");
    option.value = name;
    option.textContent = name;
    return option;
  }));
  if (select && names.includes(select)) selectEl.value = select;
  return names;
}

async function loadTheme(key, preview = true) {
  const source = await invoke("read_theme", { name: key });
  theme = normalizeTheme(JSON.parse(source));
  currentThemeKey = key;
  selectedWidget = -1;
  await refreshThemeList(key);
  renderAll();
  if (!preview) return;
  await renderPreview(true);
}

function themeKey() {
  return slug($("theme-name").value || theme?.name || currentThemeKey);
}

async function saveTheme() {
  if (!theme) throw new Error("Buat atau pilih tema terlebih dahulu.");
  const key = themeKey();
  if (!key) throw new Error("Nama file tema harus berisi huruf atau angka.");
  theme.name = $("theme-name").value.trim() || key;
  currentThemeKey = key;
  await invoke("save_theme", { name: key, json: serializableTheme() });
  await refreshThemeList(key);
  updateSource();
  toast(`Tema “${theme.name}” tersimpan.`);
  return key;
}

async function saveSettings(lastTheme = currentThemeKey) {
  await invoke("save_app_settings", {
    lastTheme,
    brightness: Number($("brightness")?.value || 60),
    orientation: $("orientation")?.value || "landscape",
    startOnBoot: $("start-on-boot").checked,
  });
}

async function refreshPorts() {
  try {
    const ports = await invoke("list_ports");
    $("ports").textContent = ports.length ? ports.join(" · ") : "Tidak ada port LCD terdeteksi";
  } catch (_) { $("ports").textContent = "Port serial tidak tersedia"; }
}

async function updateState() {
  try {
    const active = await invoke("daemon_running");
    daemonActive = active;
    $("status-dot").classList.toggle("online", active);
    $("connection-label").textContent = active ? "LCD aktif" : "LCD berhenti";
    $("sensor-state").textContent = active ? "Pembaruan sensor aktif" : "Sensor berhenti saat LCD tidak aktif";
    if (!active) renderStats(null);
  } catch (error) { toast(String(error), true); }
}

function renderStats(snapshot) {
  const values = snapshot ? [
    ["CPU", `${Math.round(snapshot.cpu.usage_percent)}%`],
    ["CPU TEMP", snapshot.cpu.temp_c == null ? "--" : `${Math.round(snapshot.cpu.temp_c)}°`],
    ["RAM", `${Math.round(snapshot.mem.used_percent)}%`],
    ["GPU", snapshot.gpus[0] ? `${Math.round(snapshot.gpus[0].usage_percent)}%` : "--"],
  ] : [["CPU", "--"], ["CPU TEMP", "--"], ["RAM", "--"], ["GPU", "--"]];
  $("stats").replaceChildren(...values.map(([label, value]) => {
    const card = document.createElement("div");
    card.className = "stat";
    const number = document.createElement("b");
    number.textContent = value;
    const caption = document.createElement("span");
    caption.textContent = label;
    card.append(number, caption);
    return card;
  }));
}

async function refreshStats() {
  if (!daemonActive) return renderStats(null);
  try { renderStats(await invoke("snapshot")); } catch (_) { renderStats(null); }
}

function startPolling() {
  clearInterval(previewTimer);
  clearInterval(statsTimer);
  previewTimer = setInterval(() => renderPreview(), 500);
  statsTimer = setInterval(refreshStats, 1800);
}

async function applyTheme() {
  const key = await saveTheme();
  const animatedFps = (theme.widgets || []).filter((item) => item.type === "image" && item.frames?.length > 1)
    .reduce((fps, item) => Math.max(fps, Number(item.fps) || MEDIA_FPS), 1);
  await invoke("daemon_stop");
  await invoke("daemon_start", {
    theme: key,
    brightness: Number($("brightness").value),
    orientation: $("orientation").value,
    fps: animatedFps,
  });
  await saveSettings(key);
  daemonActive = true;
  startPolling();
  await updateState();
  await refreshStats();
  await renderPreview(true);
  toast("Tema diterapkan ke LCD.");
}

async function stopTheme() {
  await invoke("daemon_stop");
  daemonActive = false;
  clearInterval(previewTimer);
  clearInterval(statsTimer);
  await updateState();
  renderStats(null);
  toast("Pengiriman frame dan pembacaan sensor dihentikan.");
}

function downloadJson() {
  const blob = new Blob([serializableTheme()], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = `${themeKey() || "theme"}.json`;
  document.body.append(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
  toast("JSON diekspor. Frame media tetap berada di folder aset tema.");
}

function createDefaultTheme() {
  const key = "untitled-theme";
  currentThemeKey = key;
  theme = {
    name: "Tema baru",
    width: CANVAS_SIZE,
    height: CANVAS_SIZE,
    background: { color: "#0b101e" },
    widgets: [
      { type: "clock", x: 240, y: 44, format: "%H:%M", size: 56, color: "#edf3fa", bold: true, align: "center" },
      { type: "text", x: 240, y: 274, text: "CPU {cpu.usage}%", size: 24, color: "#25e0cc", bold: true, align: "center" },
    ],
  };
  selectedWidget = -1;
  renderAll();
  $("theme-select").value = "";
}

function isVideoFile(file) {
  return file.type.startsWith("video/") || /\.(mp4|m4v|mov|webm|ogv)$/i.test(file.name);
}

function isImageFile(file) {
  return file.type.startsWith("image/") || /\.(png|jpe?g|webp|gif|svg|bmp)$/i.test(file.name);
}

async function captureMedia(file) {
  if (file.size > 80 * 1024 * 1024) throw new Error("Ukuran media maksimal 80 MB.");
  const url = URL.createObjectURL(file);
  const frames = [];
  let outputWidth = 0;
  let outputHeight = 0;
  const canvas = document.createElement("canvas");
  const context = canvas.getContext("2d", { alpha: true });
  const drawFrame = (source, width, height) => {
    const scale = Math.min(1, CANVAS_SIZE / width, CANVAS_SIZE / height);
    outputWidth = Math.max(1, Math.round(width * scale));
    outputHeight = Math.max(1, Math.round(height * scale));
    canvas.width = outputWidth;
    canvas.height = outputHeight;
    context.clearRect(0, 0, outputWidth, outputHeight);
    context.drawImage(source, 0, 0, outputWidth, outputHeight);
    frames.push(canvas.toDataURL("image/png"));
  };

  try {
    if (isVideoFile(file)) {
      const video = document.createElement("video");
      video.muted = true;
      video.playsInline = true;
      video.preload = "auto";
      const metadataReady = new Promise((resolve, reject) => {
        const timeout = setTimeout(() => reject(new Error("Waktu tunggu video habis.")), 15000);
        video.addEventListener("loadedmetadata", () => { clearTimeout(timeout); resolve(); }, { once: true });
        video.addEventListener("error", () => { clearTimeout(timeout); reject(new Error("Format video tidak didukung oleh WebView.")); }, { once: true });
      });
      video.src = url;
      await metadataReady;
      await new Promise((resolve, reject) => {
        const timeout = setTimeout(() => reject(new Error("Waktu tunggu frame video habis.")), 12000);
        video.addEventListener("loadeddata", () => { clearTimeout(timeout); resolve(); }, { once: true });
        video.addEventListener("error", () => { clearTimeout(timeout); reject(new Error("Frame video tidak dapat dibaca.")); }, { once: true });
      });
      if (!Number.isFinite(video.duration) || video.duration <= 0) throw new Error("Durasi video tidak dapat dibaca.");
      const duration = Math.min(video.duration, 6);
      const count = Math.min(MAX_MEDIA_FRAMES, Math.max(2, Math.ceil(duration * MEDIA_FPS)));
      for (let index = 0; index < count; index++) {
        const at = Math.min(duration - 0.03, (duration * index) / count);
        if (Math.abs(video.currentTime - at) > 0.01) {
          await new Promise((resolve, reject) => {
            const timeout = setTimeout(() => reject(new Error("Gagal membaca frame video.")), 12000);
            video.addEventListener("seeked", () => { clearTimeout(timeout); resolve(); }, { once: true });
            video.currentTime = at;
          });
        }
        drawFrame(video, video.videoWidth, video.videoHeight);
      }
      video.pause();
      video.removeAttribute("src");
      video.load();
    } else {
      const image = new Image();
      image.src = url;
      await image.decode();
      const animated = file.type === "image/gif" || file.name.toLowerCase().endsWith(".gif");
      const count = animated ? 8 : 1;
      for (let index = 0; index < count; index++) {
        if (index) await new Promise((resolve) => setTimeout(resolve, 500));
        drawFrame(image, image.naturalWidth, image.naturalHeight);
      }
    }
  } finally {
    URL.revokeObjectURL(url);
  }
  return { frames, width: outputWidth, height: outputHeight };
}

async function addMedia(file, x, y) {
  if (!isImageFile(file) && !isVideoFile(file)) {
    toast("Pilih gambar, GIF, atau video.", true);
    return;
  }
  try {
    toast("Menyiapkan frame media…");
    const captured = await captureMedia(file);
    const key = themeKey() || currentThemeKey || "untitled-theme";
    currentThemeKey = key;
    if (!$("theme-name").value.trim()) $("theme-name").value = key;
    const paths = await invoke("save_media_frames", { theme: key, filename: file.name, frames: captured.frames });
    const scale = Math.min(1, 280 / captured.width, 220 / captured.height);
    const w = Math.max(40, Math.round(captured.width * scale));
    const h = Math.max(40, Math.round(captured.height * scale));
    const pointX = Number.isFinite(x) ? x : dims().width / 2;
    const pointY = Number.isFinite(y) ? y : dims().height / 2;
    theme.widgets.push({ type: "image", x: Math.max(0, pointX - w / 2), y: Math.max(0, pointY - h / 2), w, h, path: paths[0], frames: paths, fps: paths.length > 1 ? MEDIA_FPS : 1 });
    selectedWidget = theme.widgets.length - 1;
    updateModel();
    toast(`${paths.length > 1 ? `${paths.length} frame` : "Media"} ditambahkan ke kanvas.`);
  } catch (error) {
    toast(String(error), true);
  }
}

function openMediaPicker() { $("media-input").click(); }
$("add-media").onclick = openMediaPicker;
$("media-drop").onclick = openMediaPicker;
$("media-input").addEventListener("change", (event) => {
  const file = event.target.files[0];
  if (file) addMedia(file);
  event.target.value = "";
});

$("theme-select").addEventListener("change", async (event) => {
  if (!event.target.value) return;
  try { await loadTheme(event.target.value); }
  catch (error) { toast(String(error), true); }
});
$("theme-name").addEventListener("input", () => {
  if (theme) { theme.name = $("theme-name").value; updateSource(); }
});
$("background-color").addEventListener("input", (event) => {
  theme.background = theme.background || {};
  theme.background.color = event.target.value;
  updateSource();
  queuePreview(true);
});
$("new-theme").onclick = createDefaultTheme;
$("save-theme").onclick = () => saveTheme().catch((error) => toast(String(error), true));
$("apply-theme").onclick = () => applyTheme().catch((error) => toast(String(error), true));
$("stop-theme").onclick = () => stopTheme().catch((error) => toast(String(error), true));
$("export-json").onclick = downloadJson;
$("toggle-json").onclick = () => {
  const panel = $("json-panel");
  panel.hidden = !panel.hidden;
  if (!panel.hidden) $("json-source").value = serializableTheme();
};
$("close-json").onclick = () => { $("json-panel").hidden = true; };
$("apply-json").onclick = () => {
  try {
    theme = normalizeTheme(JSON.parse($("json-source").value));
    selectedWidget = -1;
    renderAll();
    toast("JSON diterapkan ke kanvas.");
  } catch (error) { toast(`JSON tidak valid: ${error.message}`, true); }
};
$("import-json").onclick = () => $("json-input").click();
$("json-input").addEventListener("change", async (event) => {
  const file = event.target.files[0];
  if (!file) return;
  try {
    theme = normalizeTheme(JSON.parse(await file.text()));
    currentThemeKey = slug(file.name.replace(/\.json$/i, "")) || "imported-theme";
    selectedWidget = -1;
    await refreshThemeList();
    renderAll();
    toast("Tema JSON dibuka. Simpan untuk menambahkannya ke daftar tema.");
  } catch (error) { toast(`Gagal membuka JSON: ${error.message}`, true); }
  event.target.value = "";
});
$("start-on-boot").addEventListener("change", async () => {
  try {
    await saveSettings(currentThemeKey);
    toast($("start-on-boot").checked ? "Tema terakhir akan dimuat saat login." : "Autostart dimatikan.");
  } catch (error) { toast(String(error), true); }
});
$("brightness").addEventListener("input", () => { $("brightness-value").value = `${$("brightness").value}%`; });
$("brightness").addEventListener("change", () => saveSettings().catch((error) => toast(String(error), true)));
$("orientation").addEventListener("change", () => saveSettings().catch((error) => toast(String(error), true)));

async function init() {
  try {
    const settings = await invoke("get_app_settings");
    $("brightness").value = settings.brightness;
    $("brightness-value").value = `${settings.brightness}%`;
    $("orientation").value = settings.orientation;
    $("start-on-boot").checked = settings.startOnBoot;
    const names = await refreshThemeList();
    if (!names.length) {
      createDefaultTheme();
      await refreshPorts();
      renderStats(null);
      return;
    }
    const selected = names.includes(settings.lastTheme) ? settings.lastTheme : names[0];
    await loadTheme(selected);
    if (await invoke("daemon_running")) await stopTheme();
    await applyTheme();
    await refreshPorts();
    await refreshStats();
    setInterval(updateState, 1800);
  } catch (error) {
    toast(String(error), true);
  }
}

init();
