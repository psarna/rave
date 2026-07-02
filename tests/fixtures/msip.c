__asm__(
".section .text\n"           // Keep the raw-image entry code in the executable section.
".globl _start\n"            // Export the entry point expected by the linker script.
"_start:\n"                  // Begin in machine mode at DRAM_START.
"  la t0, trap_handler\n"    // Install a machine trap handler for software interrupts.
"  csrw mtvec, t0\n"         // Direct synchronous traps and interrupts to trap_handler.
"  li t0, (1 << 3)\n"        // Build mie.MSIE, the machine software interrupt enable bit.
"  csrw mie, t0\n"           // Enable machine software interrupts locally.
"  li t0, (1 << 3)\n"        // Build mstatus.MIE, the global machine interrupt enable bit.
"  csrs mstatus, t0\n"       // Allow enabled machine interrupts to trap.
"  li t0, 0x02000000\n"      // Load CLINT msip address for hart 0.
"  li t1, 1\n"               // Prepare the pending software-interrupt bit.
"  sw t1, 0(t0)\n"           // Raise a machine software interrupt for this hart.
"  la s0, software_seen\n"   // Remember where the handler records success.
"wait_msip:\n"               // Spin until the interrupt handler updates memory.
"  ld t0, 0(s0)\n"           // Read the software_seen flag.
"  beqz t0, wait_msip\n"     // Keep waiting while the software interrupt has not fired.
"  li t0, 0x10000000\n"      // Load the emulated UART base address.
"  li t1, 'S'\n"             // Prepare the byte proving software interrupt delivery.
"  sb t1, 0(t0)\n"           // Emit 'S' to UART.
"  li a0, 0\n"               // Report success through the emulator ebreak convention.
"  ebreak\n"                 // Stop the host emulator.
"trap_handler:\n"            // Machine software interrupt handler.
"  csrr t0, mcause\n"        // Inspect why we trapped.
"  li t1, ((1 << 63) | 3)\n" // Expected mcause: interrupt bit plus machine-software cause 3.
"  bne t0, t1, trap_fail\n"  // Fail if any other trap arrived.
"  li t0, 0x02000000\n"      // Reload CLINT msip.
"  sw zero, 0(t0)\n"         // Clear msip so the interrupt is no longer pending.
"  la t0, software_seen\n"   // Load the completion flag address.
"  li t1, 1\n"               // Prepare a nonzero completion flag.
"  sd t1, 0(t0)\n"           // Tell the main loop the software interrupt fired.
"  mret\n"                   // Return to the interrupted wait loop.
"trap_fail:\n"               // Shared failure path for unexpected traps.
"  li a0, 2\n"               // Report a distinct failure code.
"  ebreak\n"                 // Stop the emulator on failure.
".section .bss\n"            // Reserve zero-initialized guest state.
".align 3\n"                 // Align the dword flag.
"software_seen:\n"           // Flag written by the software interrupt handler.
"  .dword 0\n"               // Initially no interrupt has been observed.
);
