const MiB = 1024 * 1024;
const BOOT_MEMORY = 128 * MiB;
const RAW_MEMORY = 16 * MiB;

const presets = {
  firmware: {
    fw_jump: { label: "fw_jump (OpenSBI 1.7)", url: "./demo/fw_jump.bin" },
    shim: { label: "Minimal test boot shim", url: "./demo/boot_shim.bin" },
  },
  kernel: {
    uart_echo: { label: "uart_echo", url: "./demo/boot_payload.bin" },
  },
  raw: {
    uart: { label: "UART hello", url: "./demo/uart.bin" },
    plic: { label: "PLIC UART interrupt", url: "./demo/plic.bin" },
    sv39: { label: "Sv39 translation", url: "./demo/sv39.bin" },
    privileged: { label: "Privileged memory", url: "./demo/privileged.bin" },
    rv64c: { label: "Compressed instructions", url: "./demo/rv64c.bin" },
  },
};

const $ = (selector) => document.querySelector(selector);
const mode = $("#mode");
const firmware = $("#firmware");
const kernel = $("#kernel");
const raw = $("#raw");
const terminal = $("#terminal");
const uartInput = $("#uart-input");
const registerList = $("#register-list");
const status = $("#status");
const decoder = new TextDecoder();
const encoder = new TextEncoder();
let worker = null;

if (location.protocol === "file:") {
  setStatus("serve this folder over HTTP to boot", true);
}

fillSelect(firmware, presets.firmware);
fillSelect(kernel, presets.kernel);
fillSelect(raw, presets.raw);
mode.addEventListener("change", updateMode);
updateMode();

$("#run").addEventListener("click", start);
$("#stop").addEventListener("click", () => stop("stopped"));
$("#clear").addEventListener("click", () => { terminal.textContent = ""; });
$("#uart-form").addEventListener("submit", (event) => {
  event.preventDefault();
  if (!worker) return;
  setStatus("running");
  sendUart(`${uartInput.value}\n`);
  uartInput.value = "";
});
terminal.addEventListener("click", () => uartInput.focus());

async function start() {
  if (location.protocol === "file:") {
    setStatus("run: python3 -m http.server 8000, then open /", true);
    return;
  }
  stop();
  terminal.textContent = "";
  registerList.textContent = "";
  setStatus("loading images…");
  try {
    const message = mode.value === "raw"
      ? {
          type: "start",
          mode: "raw",
          image: await selectedBytes(raw, $("#raw-upload"), presets.raw),
          memorySize: RAW_MEMORY,
        }
      : {
          type: "start",
          mode: "boot",
          firmware: await selectedBytes(
            firmware,
            $("#firmware-upload"),
            presets.firmware,
          ),
          kernel: await selectedBytes(kernel, $("#kernel-upload"), presets.kernel),
          dtb: await fetchBytes("./demo/rave.dtb"),
          memorySize: BOOT_MEMORY,
        };
    worker = new Worker("./web/worker.js", { type: "module" });
    worker.onmessage = receive;
    worker.onerror = (event) => setStatus(event.message, true);
    const transfers = Object.values(message).filter(
      (value) => value instanceof ArrayBuffer,
    );
    worker.postMessage(message, transfers);
    uartInput.focus();
  } catch (error) {
    setStatus(error instanceof Error ? error.message : String(error), true);
  }
}

function receive({ data }) {
  if (data.type === "uart") {
    terminal.textContent += decoder.decode(data.bytes, { stream: true });
    terminal.scrollTop = terminal.scrollHeight;
  } else if (data.type === "status") {
    setStatus(data.value);
  } else if (data.type === "registers") {
    renderRegisters(data.value);
  } else if (data.type === "error") {
    setStatus(data.value, true);
  }
}

function renderRegisters(snapshot) {
  const fragment = document.createDocumentFragment();
  let group = "";
  for (const line of snapshot.trim().split("\n")) {
    const [name, value] = line.split("\t");
    const nextGroup = name === "mode" || name === "pc" || name.startsWith("x")
      ? "CPU"
      : "Pseudo-registers";
    if (nextGroup !== group) {
      group = nextGroup;
      const heading = document.createElement("h2");
      heading.textContent = group;
      fragment.append(heading);
    }
    const row = document.createElement("div");
    row.className = "register-row";
    const label = document.createElement("span");
    label.textContent = name;
    const output = document.createElement("output");
    output.textContent = value;
    row.append(label, output);
    fragment.append(row);
  }
  registerList.replaceChildren(fragment);
}

function sendUart(value) {
  if (!worker) return;
  const bytes = encoder.encode(value);
  worker.postMessage({ type: "uart", bytes }, [bytes.buffer]);
}

function stop(label) {
  worker?.terminate();
  worker = null;
  if (label) setStatus(label);
}

async function selectedBytes(select, upload, group) {
  if (upload.files[0]) return upload.files[0].arrayBuffer();
  return fetchBytes(group[select.value].url);
}

async function fetchBytes(url) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Could not load ${url}: HTTP ${response.status}`);
  }
  return response.arrayBuffer();
}

function fillSelect(select, group) {
  for (const [value, item] of Object.entries(group)) {
    select.add(new Option(item.label, value));
  }
}

function updateMode() {
  $("#boot-options").hidden = mode.value !== "boot";
  $("#raw-options").hidden = mode.value !== "raw";
}

function setStatus(value, error = false) {
  status.textContent = value;
  status.classList.toggle("error", error);
}
