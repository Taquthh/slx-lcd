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

async function loadSelected() {
  const name = $("theme").value;
  if (!name) return;
  try {
    $("editor").value = await invoke("read_theme", { name });
    $("save-name").value = name;
    say("");
    await updatePreview();
  } catch (e) {
    say(String(e), "err");
  }
}

let previewBusy = false;
async function updatePreview() {
  if (previewBusy) return;
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

$("btn-reload").onclick = loadSelected;
$("theme").onchange = loadSelected;
$("brightness").oninput = () => ($("brightness-val").textContent = $("brightness").value);

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
    // the daemon reads the theme from disk, so save the editor content first
    await invoke("save_theme", { name, json: $("editor").value });
    await refreshThemes(name);
    await invoke("daemon_stop");
    await invoke("daemon_start", {
      theme: name,
      brightness: Number($("brightness").value),
      orientation: $("orientation").value,
      fps: 1,
    });
    say("Dikirim ke layar.", "ok");
  } catch (e) {
    say(String(e), "err");
  }
  refreshState();
};

$("btn-stop").onclick = async () => {
  try {
    await invoke("daemon_stop");
    say("Dihentikan.", "ok");
  } catch (e) {
    say(String(e), "err");
  }
  refreshState();
};

async function refreshState() {
  const on = await invoke("daemon_running");
  $("dot").className = "dot" + (on ? " on" : "");
  $("state").textContent = on ? "mengirim ke layar" : "daemon berhenti";
}

const fmt = (v, d = 0, u = "") => (v == null ? "--" : v.toFixed(d) + u);

async function refreshStats() {
  try {
    const s = await invoke("snapshot");
    const g = s.gpus[0];
    const down = s.net.reduce((a, n) => a + n.rx_bytes_per_s, 0);
    const up = s.net.reduce((a, n) => a + n.tx_bytes_per_s, 0);
    const cells = [
      ["CPU", fmt(s.cpu.usage_percent, 0, "%")],
      ["Suhu CPU", fmt(s.cpu.temp_c, 0, "°C")],
      ["RAM", fmt(s.mem.used_percent, 0, "%")],
      ["GPU", g ? fmt(g.usage_percent, 0, "%") : "--"],
      ["Suhu GPU", g ? fmt(g.temp_c, 0, "°C") : "--"],
      ["Kipas", s.fans.length ? s.fans[0].rpm + " rpm" : "--"],
      ["Unduh", fmt(down / 1048576, 2, " MB/s")],
      ["Unggah", fmt(up / 1048576, 2, " MB/s")],
    ];
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
  } catch (_) {}
}

(async function init() {
  const names = await refreshThemes();
  if (names.length) await loadSelected();
  else say("Belum ada tema di ~/.config/slxd/themes. Salin themes/aqua.json ke sana atau jalankan packaging/install.sh.", "err");
  $("ports").textContent = (await invoke("list_ports")).join("\n") || "Tidak ada /dev/ttyACM* atau ttyUSB*";
  refreshState();
  refreshStats();
  setInterval(refreshStats, 2000);
  setInterval(refreshState, 2000);
  setInterval(updatePreview, 3000); // keep live values moving in the preview
})();
