const MiB = 1024 * 1024;
const BOOT_MEMORY = 128 * MiB;
const RAW_MEMORY = 16 * MiB;

const presets = {
  firmware: {
    fw_jump: { label: "fw_jump (OpenSBI 1.7)", url: "./demo/fw_jump.bin" },
    shim: { label: "Minimal test boot shim", url: "./demo/boot_shim.bin" },
  },
  kernel: {
    linux: {
      label: "Linux 6.18.7 + Buildroot",
      url: "./demo/linux/Image?v=20260723-1",
      initrdUrl: "./demo/linux/rootfs.cpio?v=20260723-1",
    },
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
const terminal = $("#terminal-output");
const terminalViewport = $("#terminal");
const uartInput = $("#uart-input");
const registerList = $("#register-list");
const status = $("#status");
let decoder = new TextDecoder();
const encoder = new TextEncoder();
let ansiTerminal;
let worker = null;
const uartHistory = [];
let uartHistoryIndex = 0;
let uartHistoryDraft = "";
let uartStagedLine = "";
let uartLineOnGuest = false;

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
$("#clear").addEventListener("click", () => ansiTerminal.clear());
$("#uart-form").addEventListener("submit", (event) => {
  event.preventDefault();
  if (!worker) return;
  const line = uartStagedLine + uartInput.value;
  if (line && uartHistory.at(-1) !== line) uartHistory.push(line);
  uartHistoryIndex = uartHistory.length;
  uartHistoryDraft = "";
  uartStagedLine = "";
  uartLineOnGuest = false;
  setStatus("running");
  sendUart(`${uartInput.value}\n`);
  uartInput.value = "";
});
uartInput.addEventListener("keydown", (event) => {
  if (event.key === "Tab") completeUartInput(event);
  else if (event.key === "Backspace") backspaceUartInput(event);
  else if (event.key === "ArrowUp") recallUartHistory(-1, event);
  else if (event.key === "ArrowDown") recallUartHistory(1, event);
});
terminalViewport.addEventListener("click", () => uartInput.focus());

async function start() {
  if (location.protocol === "file:") {
    setStatus("run: python3 -m http.server 8000, then open /", true);
    return;
  }
  stop();
  decoder = new TextDecoder();
  ansiTerminal.clear();
  uartInput.value = "";
  uartStagedLine = "";
  uartLineOnGuest = false;
  uartHistoryIndex = uartHistory.length;
  uartHistoryDraft = "";
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
          initrd: await selectedInitrd(
            $("#initrd-upload"),
            presets.kernel[kernel.value],
          ),
          dtb: await fetchBytes("./demo/rave.dtb"),
          memorySize: BOOT_MEMORY,
        };
    worker = new Worker("./web/worker.js?v=20260723-1", { type: "module" });
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
    ansiTerminal.write(decoder.decode(data.bytes, { stream: true }));
    terminalViewport.scrollTop = terminalViewport.scrollHeight;
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

function completeUartInput(event) {
  event.preventDefault();
  if (!worker) return;
  uartStagedLine += uartInput.value;
  uartLineOnGuest = true;
  sendUart(`${uartInput.value}\t`);
  uartInput.value = "";
  uartHistoryIndex = uartHistory.length;
  uartHistoryDraft = "";
}

function backspaceUartInput(event) {
  if (!worker || !uartLineOnGuest || uartInput.value !== "") return;
  event.preventDefault();
  sendUart("\x7f");
  uartStagedLine = uartStagedLine.slice(0, -1);
}

function recallUartHistory(direction, event) {
  if (uartHistory.length === 0) return;
  event.preventDefault();

  if (direction < 0) {
    if (uartHistoryIndex === uartHistory.length) uartHistoryDraft = uartInput.value;
    uartHistoryIndex = Math.max(0, uartHistoryIndex - 1);
    uartInput.value = uartHistory[uartHistoryIndex];
  } else {
    uartHistoryIndex = Math.min(uartHistory.length, uartHistoryIndex + 1);
    uartInput.value = uartHistoryIndex === uartHistory.length
      ? uartHistoryDraft
      : uartHistory[uartHistoryIndex];
  }
  uartInput.setSelectionRange(uartInput.value.length, uartInput.value.length);
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

async function selectedInitrd(upload, kernelPreset) {
  if (upload.files[0]) return upload.files[0].arrayBuffer();
  return kernelPreset.initrdUrl ? fetchBytes(kernelPreset.initrdUrl) : null;
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

const ANSI_COLORS = [
  "#263640", "#ef6b73", "#65d18c", "#e8c66a",
  "#62a8ea", "#c792ea", "#56d6d0", "#d6e2e4",
  "#607985", "#ff7b85", "#7ee8a5", "#f3d984",
  "#82bfff", "#d9a4ff", "#76eee8", "#ffffff",
];

class AnsiTerminal {
  constructor(element) {
    this.element = element;
    this.pending = "";
    this.pendingCarriageReturn = false;
    this.resetStyle();
  }

  clear() {
    this.element.replaceChildren();
    this.pending = "";
    this.pendingCarriageReturn = false;
    this.resetStyle();
  }

  write(text) {
    this.pending += text;
    let offset = 0;

    while (offset < this.pending.length) {
      const escape = this.pending.indexOf("\x1b", offset);
      if (escape === -1) {
        this.append(this.pending.slice(offset));
        this.pending = "";
        return;
      }

      this.append(this.pending.slice(offset, escape));
      if (escape + 1 >= this.pending.length) {
        this.pending = this.pending.slice(escape);
        return;
      }

      if (this.pending[escape + 1] !== "[") {
        this.flushCarriageReturn();
        // Consume unsupported two-byte escape commands instead of printing them.
        offset = escape + 2;
        continue;
      }

      let end = escape + 2;
      while (end < this.pending.length) {
        const code = this.pending.charCodeAt(end);
        if (code >= 0x40 && code <= 0x7e) break;
        end += 1;
      }
      if (end === this.pending.length) {
        this.pending = this.pending.slice(escape);
        return;
      }

      const parameters = this.pending.slice(escape + 2, end);
      const command = this.pending[end];
      this.flushCarriageReturn();
      if (command === "m") this.applySgr(parameters);
      if (command === "J" && (parameters === "2" || parameters === "3")) {
        this.element.replaceChildren();
      }
      offset = end + 1;
    }

    this.pending = "";
  }

  resetStyle() {
    this.style = {
      bold: false,
      dim: false,
      underline: false,
      reverse: false,
      foreground: null,
      background: null,
    };
  }

  applySgr(parameters) {
    const values = parameters === ""
      ? [0]
      : parameters.split(";").map((value) => value === "" ? 0 : Number(value));

    for (let index = 0; index < values.length; index += 1) {
      const value = values[index];
      if (value === 0) this.resetStyle();
      else if (value === 1) this.style.bold = true;
      else if (value === 2) this.style.dim = true;
      else if (value === 4) this.style.underline = true;
      else if (value === 7) this.style.reverse = true;
      else if (value === 22) {
        this.style.bold = false;
        this.style.dim = false;
      } else if (value === 24) this.style.underline = false;
      else if (value === 27) this.style.reverse = false;
      else if (value >= 30 && value <= 37) this.style.foreground = value - 30;
      else if (value === 39) this.style.foreground = null;
      else if (value >= 40 && value <= 47) this.style.background = value - 40;
      else if (value === 49) this.style.background = null;
      else if (value >= 90 && value <= 97) this.style.foreground = value - 90 + 8;
      else if (value >= 100 && value <= 107) this.style.background = value - 100 + 8;
      else if ((value === 38 || value === 48) && values[index + 1] === 5) {
        const color = ansi256(values[index + 2]);
        if (color !== null) this.setColor(value, color);
        index += 2;
      } else if ((value === 38 || value === 48) && values[index + 1] === 2) {
        const rgb = values.slice(index + 2, index + 5);
        if (rgb.length === 3 && rgb.every((part) => part >= 0 && part <= 255)) {
          this.setColor(value, `rgb(${rgb.join(", ")})`);
        }
        index += 4;
      }
    }
  }

  setColor(command, color) {
    if (command === 38) this.style.foreground = color;
    else this.style.background = color;
  }

  append(text) {
    if (!text) return;
    let start = 0;
    for (let index = 0; index < text.length; index += 1) {
      const character = text[index];
      if (this.pendingCarriageReturn) {
        this.pendingCarriageReturn = false;
        if (character === "\n") {
          this.appendStyled(text.slice(start, index) + "\n");
          start = index + 1;
          continue;
        }
        this.carriageReturn();
      }
      if (character === "\r" || character === "\x07" || character === "\x08") {
        this.appendStyled(text.slice(start, index));
        start = index + 1;
        if (character === "\r") this.pendingCarriageReturn = true;
        else if (character === "\x08") this.backspace();
      }
    }
    this.appendStyled(text.slice(start));
  }

  flushCarriageReturn() {
    if (!this.pendingCarriageReturn) return;
    this.pendingCarriageReturn = false;
    this.carriageReturn();
  }

  carriageReturn() {
    while (this.element.lastChild) {
      const node = this.element.lastChild;
      const text = node.textContent;
      const newline = text.lastIndexOf("\n");
      if (newline === -1) {
        node.remove();
        continue;
      }
      const retained = text.slice(0, newline + 1);
      if (node.nodeType === Node.TEXT_NODE) node.data = retained;
      else node.firstChild.data = retained;
      return;
    }
  }

  backspace() {
    const node = this.element.lastChild;
    if (!node || node.textContent.endsWith("\n")) return;
    const shortened = Array.from(node.textContent).slice(0, -1).join("");
    if (!shortened) node.remove();
    else if (node.nodeType === Node.TEXT_NODE) node.data = shortened;
    else node.firstChild.data = shortened;
  }

  appendStyled(text) {
    if (!text) return;
    const foreground = resolveColor(this.style.foreground, this.style.bold);
    const background = resolveColor(this.style.background, false);
    const isDefault = !this.style.bold && !this.style.dim &&
      !this.style.underline && !this.style.reverse &&
      foreground === null && background === null;

    if (isDefault) {
      const last = this.element.lastChild;
      if (last?.nodeType === Node.TEXT_NODE) last.appendData(text);
      else this.element.append(document.createTextNode(text));
      return;
    }

    let color = foreground;
    let backgroundColor = background;
    if (this.style.reverse) {
      color = backgroundColor ?? "#03090d";
      backgroundColor = foreground ?? "#8bf1e9";
    }
    const signature = JSON.stringify([
      color, backgroundColor, this.style.bold, this.style.dim, this.style.underline,
    ]);
    const last = this.element.lastElementChild;
    if (last?.dataset.ansiStyle === signature && last === this.element.lastChild) {
      last.firstChild.appendData(text);
      return;
    }

    const span = document.createElement("span");
    span.dataset.ansiStyle = signature;
    if (color !== null) span.style.color = color;
    if (backgroundColor !== null) span.style.backgroundColor = backgroundColor;
    if (this.style.bold) span.style.fontWeight = "700";
    if (this.style.dim) span.style.opacity = "0.65";
    if (this.style.underline) span.style.textDecoration = "underline";
    span.append(document.createTextNode(text));
    this.element.append(span);
  }
}

function resolveColor(color, bold) {
  if (typeof color === "number") {
    const index = bold && color < 8 ? color + 8 : color;
    return ANSI_COLORS[index];
  }
  return color;
}

function ansi256(value) {
  if (!Number.isInteger(value) || value < 0 || value > 255) return null;
  if (value < 16) return ANSI_COLORS[value];
  if (value < 232) {
    const level = [0, 95, 135, 175, 215, 255];
    const offset = value - 16;
    const red = level[Math.floor(offset / 36)];
    const green = level[Math.floor((offset % 36) / 6)];
    const blue = level[offset % 6];
    return `rgb(${red}, ${green}, ${blue})`;
  }
  const gray = 8 + (value - 232) * 10;
  return `rgb(${gray}, ${gray}, ${gray})`;
}

ansiTerminal = new AnsiTerminal(terminal);
