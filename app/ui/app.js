// SPDX-License-Identifier: GPL-3.0-or-later
// Vanilla JS front end (no build step). Talks to the Rust side through Tauri's global invoke.
const invoke = window.__TAURI__.core.invoke;
const $ = (id) => document.getElementById(id);

function say(text, kind) {
  const m = $("msg");
  m.textContent = text || "";
  m.className = kind || "";
}

async function refreshThemes(select) {
  const names = await invoke("list_themes");
  const sel = $("theme");
  sel.replaceChildren(...names.map((n) => Object.assign(document.createElement("option"), { value: n, textContent: n })));
  if (select && names.includes(select)) sel.value = select;
  return names;
}

let lastAppliedTheme = "";

async function loadSelected() {
  const name = $("theme").value;
  if (!name) return;
  try {
    $("editor").value = await invoke("read_theme", { name });
    $("save-name").value = name;
    say("");
    await updatePreview(true);
  } catch (e) {
    say(String(e), "err");
  }
}

async function saveSettings(theme) {
  await invoke("save_app_settings", {
    lastTheme: theme,
    brightness: Number($("brightness").value),
    orientation: $("orientation").value,
    startOnBoot: $("start-on-boot").checked,
  });
}

async function startTheme(name, saveEditor) {
  if (saveEditor) {
    await invoke("save_theme", { name, json: $("editor").value });
    await refreshThemes(name);
  }
  await invoke("daemon_stop");
  await invoke("daemon_start", {
    theme: name,
    brightness: Number($("brightness").value),
    orientation: $("orientation").value,
    fps: 1,
  });
  await saveSettings(name);
  lastAppliedTheme = name;
  daemonActive = true;
  say("Tema dimuat. Aplikasi sedang mencoba menghubungkan LCD; periksa port dan izin jika layar belum berubah.", "ok");
  await refreshState();
  await refreshStats();
  await updatePreview();
}

async function saveFormSettings() {
  const theme = lastAppliedTheme || $("theme").value;
  if (theme) await saveSettings(theme);
}

let daemonActive = false;
let previewBusy = false;
async function updatePreview(force = false) {
  if ((!daemonActive && !force) || previewBusy) return;
  previewBusy = true;
  try {
    $("preview").src = await invoke("preview", { themeJson: $("editor").value });
    if ($("msg").className === "err") say("");
  } catch (e) {
    say(String(e), "err"); // invalid JSON / color etc.: keep the last good preview on screen
  } finally {
    previewBusy = false;
  }
}

let debounce;
$("editor").addEventListener("input", () => {
  clearTimeout(debounce);
  debounce = setTimeout(updatePreview, 400);
});

$("theme").onchange = () => loadSelected(false);
$("brightness").oninput = () => ($("brightness-val").textContent = $("brightness").value);
$("brightness").onchange = () => saveFormSettings().catch((e) => say(String(e), "err"));
$("orientation").onchange = () => saveFormSettings().catch((e) => say(String(e), "err"));
$("start-on-boot").onchange = async () => {
  const checkbox = $("start-on-boot");
  try {
    await saveFormSettings();
    say(checkbox.checked ? "Tema terakhir akan dimuat saat sistem mulai." : "Muat otomatis saat sistem mulai dinonaktifkan.", "ok");
  } catch (e) {
    checkbox.checked = !checkbox.checked;
    say(String(e), "err");
  }
};

$("btn-save").onclick = async () => {
  const name = $("save-name").value.trim();
  try {
    await invoke("save_theme", { name, json: $("editor").value });
    await refreshThemes(name);
    say(`Tersimpan: ${name}`, "ok");
  } catch (e) {
    say(String(e), "err");
  }
};

$("btn-start").onclick = async () => {
  const name = $("save-name").value.trim() || $("theme").value;
  try {
    await startTheme(name, true);
  } catch (e) {
    say(String(e), "err");
  }
};

$("btn-stop").onclick = async () => {
  try {
    await invoke("daemon_stop");
    daemonActive = false;
    await refreshState();
    await refreshStats();
    say("Pengiriman ke LCD dan pembacaan sensor dihentikan.", "ok");
  } catch (e) {
    say(String(e), "err");
  }
};

async function refreshState() {
  try {
    const on = await invoke("daemon_running");
    const wasActive = daemonActive;
    daemonActive = on;
    $("dot").className = "dot" + (on ? " on" : "");
    $("state").textContent = on ? "daemon aktif; mencoba LCD" : "daemon berhenti";
    if (wasActive && !on) await refreshStats();
  } catch (e) {
    say(String(e), "err");
  }
}

const fmt = (v, d = 0, u = "") => (v == null ? "--" : v.toFixed(d) + u);

function renderStats(cells) {
  $("stats").replaceChildren(
    ...cells.map(([k, v]) => {
      const d = document.createElement("div");
      d.className = "stat";
      const b = document.createElement("b");
      b.textContent = v;
      const sp = document.createElement("span");
      sp.textContent = k;
      d.append(b, sp);
      return d;
    })
  );
}

async function refreshStats() {
  if (!daemonActive) {
    renderStats([["CPU", "--"], ["Suhu CPU", "--"], ["RAM", "--"], ["GPU", "--"], ["Suhu GPU", "--"], ["Kipas", "--"], ["Unduh", "--"], ["Unggah", "--"]]);
    return;
  }
  try {
    const s = await invoke("snapshot");
    const g = s.gpus[0];
    const down = s.net.reduce((a, n) => a + n.rx_bytes_per_s, 0);
    const up = s.net.reduce((a, n) => a + n.tx_bytes_per_s, 0);
    renderStats([
      ["CPU", fmt(s.cpu.usage_percent, 0, "%")],
      ["Suhu CPU", fmt(s.cpu.temp_c, 0, "°C")],
      ["RAM", fmt(s.mem.used_percent, 0, "%")],
      ["GPU", g ? fmt(g.usage_percent, 0, "%") : "--"],
      ["Suhu GPU", g ? fmt(g.temp_c, 0, "°C") : "--"],
      ["Kipas", s.fans.length ? s.fans[0].rpm + " rpm" : "--"],
      ["Unduh", fmt(down / 1048576, 2, " MB/s")],
      ["Unggah", fmt(up / 1048576, 2, " MB/s")],
    ]);
  } catch (_) {}
}

(async function init() {
  try {
    const settings = await invoke("get_app_settings");
    $("brightness").value = settings.brightness;
    $("brightness-val").textContent = settings.brightness;
    $("orientation").value = settings.orientation;
    $("start-on-boot").checked = settings.startOnBoot;

    const names = await refreshThemes();
    const theme = names.includes(settings.lastTheme) ? settings.lastTheme : names[0];
    if (theme) {
      await refreshThemes(theme);
      await loadSelected();
      await startTheme(theme, false);
    } else {
      say("Belum ada tema di ~/.config/slxd/themes. Jalankan packaging/install-gui.sh terlebih dahulu.", "err");
    }
    $("ports").textContent = (await invoke("list_ports")).join("\n") || "Tidak ada /dev/ttyACM* atau ttyUSB*";
    refreshState();
    refreshStats();
    setInterval(refreshStats, 2000);
    setInterval(refreshState, 2000);
    setInterval(updatePreview, 3000);
  } catch (e) {
    say(String(e), "err");
  }
})();
