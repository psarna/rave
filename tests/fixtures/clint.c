__asm__(
".section .text\n"           // Keep the raw-image entry code in the executable section.
".globl _start\n"            // Export the entry point expected by the linker script.
"_start:\n"                  // Begin in machine mode at DRAM_START.
"  la t0, trap_handler\n"    // Install a machine trap handler for the timer interrupt.
"  csrw mtvec, t0\n"         // Direct synchronous traps and interrupts to trap_handler.
"  li t0, 0x0200bff8\n"      // Load CLINT mtime address.
"  ld t1, 0(t0)\n"           // Read the current machine time.
"  addi t1, t1, 3\n"         // Choose a near deadline so the test completes quickly.
"  li t0, 0x02004000\n"      // Load CLINT mtimecmp address for hart 0.
"  sd t1, 0(t0)\n"           // Program the machine timer interrupt deadline.
"  li t0, (1 << 7)\n"        // Build mie.MTIE, the machine timer interrupt enable bit.
"  csrw mie, t0\n"           // Enable machine timer interrupts locally.
"  li t0, (1 << 3)\n"        // Build mstatus.MIE, the global machine interrupt enable bit.
"  csrs mstatus, t0\n"       // Allow enabled machine interrupts to trap.
"  la s0, timer_seen\n"      // Remember where the handler records success.
"wait_timer:\n"              // Spin until the interrupt handler updates memory.
"  ld t0, 0(s0)\n"           // Read the timer_seen flag.
"  beqz t0, wait_timer\n"    // Keep waiting while the timer interrupt has not fired.
"  li t0, 0x10000000\n"      // Load the emulated UART base address.
"  li t1, 'I'\n"             // Prepare the byte proving interrupt delivery.
"  sb t1, 0(t0)\n"           // Emit 'I' to UART.
"  li a0, 0\n"               // Report success through the emulator ebreak convention.
"  ebreak\n"                 // Stop the host emulator.
"trap_handler:\n"            // Machine timer interrupt handler.
"  csrr t0, mcause\n"        // Inspect why we trapped.
"  li t1, ((1 << 63) | 7)\n" // Expected mcause: interrupt bit plus machine-timer cause 7.
"  bne t0, t1, trap_fail\n"  // Fail if any other trap arrived.
"  la t0, timer_seen\n"      // Load the completion flag address.
"  li t1, 1\n"               // Prepare a nonzero completion flag.
"  sd t1, 0(t0)\n"           // Tell the main loop the timer interrupt fired.
"  li t0, 0x02004000\n"      // Reload mtimecmp.
"  li t1, -1\n"              // Disable further timer interrupts by setting a far deadline.
"  sd t1, 0(t0)\n"           // Clear the pending timer condition.
"  mret\n"                   // Return to the interrupted wait loop.
"trap_fail:\n"               // Shared failure path for unexpected traps.
"  li a0, 2\n"               // Report a distinct failure code.
"  ebreak\n"                 // Stop the emulator on failure.
".section .bss\n"            // Reserve zero-initialized guest state.
".align 3\n"                 // Align the dword flag.
"timer_seen:\n"              // Flag written by the timer interrupt handler.
"  .dword 0\n"               // Initially no interrupt has been observed.
);
