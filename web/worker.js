import init, { WasmMachine } from "./pkg/rave.js?v=20260718-1";

let machine = null;
let generation = 0;
let chunkCount = 0;
const CHUNK_SIZE = 50_000;

self.onmessage = async ({ data }) => {
  try {
    if (data.type === "start") {
      const current = ++generation;
      await init();
      machine?.free();
      machine = data.mode === "raw"
        ? WasmMachine.raw(new Uint8Array(data.image), data.memorySize)
        : WasmMachine.boot(
            new Uint8Array(data.firmware),
            new Uint8Array(data.kernel),
            new Uint8Array(data.dtb),
            data.memorySize,
          );
      chunkCount = 0;
      self.postMessage({ type: "status", value: "running" });
      postRegisters();
      tick(current);
    } else if (data.type === "uart" && machine) {
      machine.push_uart_input(new Uint8Array(data.bytes));
    }
  } catch (error) {
    self.postMessage({ type: "error", value: errorText(error) });
  }
};

function tick(current) {
  if (!machine || current !== generation) return;
  try {
    const state = machine.run_chunk(CHUNK_SIZE);
    chunkCount += 1;
    const output = machine.take_uart_output();
    if (output.length) {
      self.postMessage({ type: "uart", bytes: output }, [output.buffer]);
    }
    if (chunkCount % 10 === 0 || state !== "running") {
      postRegisters();
    }
    if (state.startsWith("halted:")) {
      self.postMessage({ type: "status", value: state });
      return;
    }
    if (state === "waiting") {
      self.postMessage({ type: "status", value: "waiting for UART" });
    }
    setTimeout(() => tick(current), 0);
  } catch (error) {
    self.postMessage({ type: "error", value: errorText(error) });
  }
}

function postRegisters() {
  if (typeof machine.register_snapshot !== "function") {
    throw new Error("Browser assets are out of sync; reload while bypassing the cache");
  }
  self.postMessage({ type: "registers", value: machine.register_snapshot() });
}

function errorText(error) {
  return error instanceof Error ? error.message : String(error);
}
