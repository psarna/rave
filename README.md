# rave

![demo](demo/screen.png)

A minimal RV64I (more letters coming!) emulator

- 32 integer registers and an explicit program counter
- base RV64I integer, branch, jump, and load/store instructions
- RV64 word operations with 32-bit sign extension
- raw binaries loaded into DRAM at `0x8000_0000`
- `ebreak` as a temporary host exit boundary; register `a0` is the result code

Run a raw guest image with:

```sh
cargo run -- path/to/guest.bin
```

## Debugging HQ

Launch the interactive debugger with:

```sh
cargo run -- --interactive path/to/guest.bin
```

The debugger accepts `start`, `step`, `next`, `break ADDR`, and `continue`
(`r`, `s`, `n`, `b`, and `c` aliases are available). Use Tab to select the
register pane, arrow keys to choose a register, and Enter to edit it. F5, F10,
and F11 provide continue, next, and step shortcuts. Register edits update the
display as they are typed; use `u`, `undo`, or Ctrl-Z to restore the previous
value. Press `q` to quit. Ctrl-C and Ctrl-D require two presses within one
second, reducing accidental exits. Press Enter on an empty command prompt to
repeat the previous command, as in GDB. Before any command has been entered,
Enter defaults to `step`.

Conditional branches show their operands, evaluated condition, taken/not-taken
state, and target address. When the current branch target is visible in the
code pane, an ASCII gutter arrow connects the instruction to its destination.

The integration test compiles `tests/fixtures/guest.c` as freestanding RV64I
code with no standard library, links it at the fixed load address, converts it
to a raw binary, and runs it in the emulator:

```sh
cargo test
```

The address map reserves ROM at `0x1000`, CLINT at `0x0200_0000`, PLIC at
`0x0c00_0000`, UART at `0x1000_0000`, virtio at `0x1000_1000`, and DRAM at
`0x8000_0000`. Device regions are explicit stubs for now.

Compressed instructions, privileged modes, CSRs, virtual memory,
interrupts, and functional devices are intentionally not here yet.
