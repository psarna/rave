use crate::{Machine, CSR_MCOUNTEREN, CSR_MSTATUS, CSR_SATP, CSR_SCOUNTEREN};
use std::fmt::Write;
use wasm_bindgen::prelude::*;

/// Browser-facing wrapper around the platform emulator.
///
/// JavaScript owns one of these inside a Web Worker and calls `run_chunk`
/// repeatedly. Chunking lets UART input and stop requests arrive between
/// batches of guest instructions.
#[wasm_bindgen]
pub struct WasmMachine {
    machine: Machine,
    uart_cursor: usize,
}

#[wasm_bindgen]
impl WasmMachine {
    #[wasm_bindgen(js_name = raw)]
    pub fn raw(image: &[u8], memory_size: usize) -> Result<WasmMachine, JsValue> {
        let machine =
            Machine::from_raw(image, Machine::LOAD_ADDRESS, memory_size).map_err(js_error)?;
        Ok(Self {
            machine,
            uart_cursor: 0,
        })
    }

    #[wasm_bindgen(js_name = boot)]
    pub fn boot(
        firmware: &[u8],
        kernel: &[u8],
        device_tree: &[u8],
        memory_size: usize,
    ) -> Result<WasmMachine, JsValue> {
        let machine =
            Machine::from_boot(firmware, kernel, device_tree, memory_size).map_err(js_error)?;
        Ok(Self {
            machine,
            uart_cursor: 0,
        })
    }

    /// Runs at most `instructions` guest instructions.
    ///
    /// Returns `running`, `waiting`, or `halted:<debug reason>`. A UART wait
    /// is advisory: callers may continue ticking a guest that polls the UART.
    pub fn run_chunk(&mut self, instructions: u32) -> Result<String, JsValue> {
        for _ in 0..instructions {
            if let Some(reason) = self.machine.step().map_err(js_error)? {
                return Ok(format!("halted:{reason:?}"));
            }
        }
        if self.machine.bus.take_uart_input_wait() {
            Ok("waiting".into())
        } else {
            Ok("running".into())
        }
    }

    pub fn push_uart_input(&mut self, input: &[u8]) {
        self.machine.bus.push_uart_input(input);
    }

    /// Copies only UART bytes produced since the previous call.
    pub fn take_uart_output(&mut self) -> Vec<u8> {
        let output = self.machine.bus.uart_output();
        let bytes = output[self.uart_cursor..].to_vec();
        self.uart_cursor = output.len();
        bytes
    }

    /// Returns a tab-separated snapshot for the browser register panel.
    pub fn register_snapshot(&self) -> String {
        const ABI_NAMES: [&str; 32] = [
            "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1", "a0", "a1", "a2", "a3",
            "a4", "a5", "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11",
            "t3", "t4", "t5", "t6",
        ];

        let mut snapshot = String::new();
        writeln!(
            snapshot,
            "mode\t{}",
            self.machine.cpu.privilege_mode().label()
        )
        .unwrap();
        writeln!(snapshot, "pc\t{:#018x}", self.machine.cpu.pc).unwrap();
        for (index, abi_name) in ABI_NAMES.iter().enumerate() {
            writeln!(
                snapshot,
                "x{index}/{abi_name}\t{:#018x}",
                self.machine.cpu.register(index)
            )
            .unwrap();
        }

        let cpu = &self.machine.cpu;
        let bus = &self.machine.bus;
        for (name, value) in [
            ("mstatus", cpu.csr(CSR_MSTATUS)),
            ("satp", cpu.csr(CSR_SATP)),
            ("mcounteren", cpu.csr(CSR_MCOUNTEREN)),
            ("scounteren", cpu.csr(CSR_SCOUNTEREN)),
            ("msip", bus.msip()),
            ("mtime", bus.mtime()),
            ("mtimecmp", bus.mtimecmp()),
            ("uart.ier", bus.uart_interrupt_enable()),
            ("plic.uart_priority", bus.plic_uart_priority()),
            ("plic.pending", bus.plic_pending()),
            ("plic.m.enable", bus.plic_machine_enable()),
            ("plic.m.threshold", bus.plic_machine_threshold()),
            ("plic.s.enable", bus.plic_supervisor_enable()),
            ("plic.s.threshold", bus.plic_supervisor_threshold()),
        ] {
            writeln!(snapshot, "{name}\t{value:#018x}").unwrap();
        }
        snapshot
    }
}

fn js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}
