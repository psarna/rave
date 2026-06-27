# rave

![logo](demo/rave.png)

A minimal RV64IM_Zicsr_Zifencei (more letters coming!) emulator

- 32 integer registers and an explicit program counter
- base RV64I integer, branch, jump, and load/store instructions
- RV64M integer multiply, divide, and remainder instructions
- Zicsr CSR read/write, set, and clear instructions
- Zifencei instruction-fetch fence as a validated no-op
- RV64 word operations with 32-bit sign extension
- raw binaries loaded into DRAM at `0x8000_0000`
- `ebreak` as a temporary host exit boundary; register `a0` is the result code
- polled 16550-style UART input and output at `0x1000_0000`

Run a raw guest image with:

```sh
cargo run -- path/to/guest.bin
```

When stdin is piped in non-interactive mode, the bytes are queued as UART
receive data for the guest.

## Debugging HQ

![demo](demo/screen.png)

Launch the interactive debugger with:

```sh
cargo run -- --interactive path/to/guest.bin
```

The debugger accepts `start`, `step`, `next`, `break ADDR`, `continue`, and `uart TEXT`
(`r`, `s`, `n`, `b`, and `c` aliases are available). Use Tab to select the
register pane, arrow keys to choose a register, and Enter to edit it. F5, F10,
and F11 provide continue, next, and step shortcuts; F6 opens UART input.

Use `u`, `undo`, or Ctrl-Z to restore the previous register value.
Press `q`, double-Ctrl-C, or double-Ctrl-D to quit.
If you're a man of culture, press Enter on an empty command prompt
to repeat the previous command, gdb-style.
Initial Enter defaults to `step`.

See `tests/fixtures` for integration tests that compile C with a risc-v
target. See `demo/` for a few precompiled ones. Run with e.g.

```
cargo run -- --interactive demo/uart.bin
```

Auto tests:
```sh
cargo test
```

Compressed instructions, privileged modes, virtual memory,
interrupts, and functional devices are intentionally not here yet. They will come.
