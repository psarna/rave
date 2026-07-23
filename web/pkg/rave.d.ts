/* tslint:disable */
/* eslint-disable */

/**
 * Browser-facing wrapper around the platform emulator.
 *
 * JavaScript owns one of these inside a Web Worker and calls `run_chunk`
 * repeatedly. Chunking lets UART input and stop requests arrive between
 * batches of guest instructions.
 */
export class WasmMachine {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    static boot(firmware: Uint8Array, kernel: Uint8Array, device_tree: Uint8Array, memory_size: number): WasmMachine;
    static bootWithInitrd(firmware: Uint8Array, kernel: Uint8Array, initrd: Uint8Array, device_tree: Uint8Array, memory_size: number): WasmMachine;
    push_uart_input(input: Uint8Array): void;
    static raw(image: Uint8Array, memory_size: number): WasmMachine;
    /**
     * Returns a tab-separated snapshot for the browser register panel.
     */
    register_snapshot(): string;
    /**
     * Runs at most `instructions` guest instructions.
     *
     * Returns `running`, `waiting`, or `halted:<debug reason>`. A UART wait
     * is advisory: callers may continue ticking a guest that polls the UART.
     */
    run_chunk(instructions: number): string;
    /**
     * Copies only UART bytes produced since the previous call.
     */
    take_uart_output(): Uint8Array;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_wasmmachine_free: (a: number, b: number) => void;
    readonly wasmmachine_boot: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number, number];
    readonly wasmmachine_bootWithInitrd: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => [number, number, number];
    readonly wasmmachine_push_uart_input: (a: number, b: number, c: number) => void;
    readonly wasmmachine_raw: (a: number, b: number, c: number) => [number, number, number];
    readonly wasmmachine_register_snapshot: (a: number) => [number, number];
    readonly wasmmachine_run_chunk: (a: number, b: number) => [number, number, number, number];
    readonly wasmmachine_take_uart_output: (a: number) => [number, number];
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
