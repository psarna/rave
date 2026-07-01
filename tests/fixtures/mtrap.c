__asm__(
".section .text\n"          // Place the whole guest in the executable text section.
".globl _start\n"           // Export the raw-image entry point expected by the linker script.
"_start:\n"                 // Begin execution at DRAM_START with the CPU in machine mode.
"  la t0, trap_handler\n"   // Load the address of our machine-mode trap handler into t0.
"  csrw mtvec, t0\n"        // Point mtvec at the handler so the following ecall traps there.
"  li a0, 1\n"              // Seed a nonzero exit code; success must overwrite this later.
"  ecall\n"                 // Raise an environment-call-from-M-mode exception.
"  ebreak\n"                // Failure guard: if ecall does not trap/return past this, exit with code 1.
"after_ecall:\n"            // Resume point selected by the trap handler through mepc.
"  li t0, 0x10000000\n"     // Load the emulated UART base address while still in machine mode.
"  li t1, 'T'\n"            // Prepare the byte that proves the trap handler returned here.
"  sb t1, 0(t0)\n"          // Write 'T' to UART transmit holding register.
"  la t0, user_entry\n"     // Load the address that should run after dropping privilege.
"  csrw mepc, t0\n"         // Build an artificial mret frame targeting user_entry.
"  csrr t0, mstatus\n"      // Read mstatus so we can edit the MPP field.
"  li t1, ~(3 << 11)\n"     // Build a mask that clears mstatus.MPP bits 12:11.
"  and t0, t0, t1\n"       // Set MPP=U, since user mode is encoded as zero.
"  csrw mstatus, t0\n"      // Commit the privilege-drop return state.
"  mret\n"                  // Enter user mode at user_entry using the artificial frame.
"  li a0, 3\n"              // Failure guard: reaching here means mret did not jump to user_entry.
"  ebreak\n"                // Stop with a distinct failure code if the user-mode mret failed.
"user_entry:\n"             // Code reached after mret lowers current privilege to U-mode.
"  li t0, 0x10000000\n"     // Reload UART base; registers are not part of the privilege transition contract.
"  li t1, 'U'\n"            // Prepare the byte proving execution continued after the U-mode drop.
"  sb t1, 0(t0)\n"          // Write 'U' to UART from the lowered privilege mode.
"  li a0, 0\n"              // Set host-observed ebreak result code to success.
"  ebreak\n"                // Stop the emulator through the temporary host exit boundary.
"trap_handler:\n"           // Machine trap entry reached via mtvec.
"  csrr t0, mcause\n"       // Read the trap cause produced by the ecall.
"  li t1, 11\n"             // mcause 11 is environment call from machine mode.
"  bne t0, t1, trap_fail\n" // Any other cause means trap entry is wrong, so fail visibly.
"  csrr t0, mepc\n"         // Read the trapped instruction address.
"  addi t0, t0, 8\n"        // Skip both the 4-byte ecall and the 4-byte failure-guard ebreak.
"  csrw mepc, t0\n"         // Store the post-trap resume address for mret.
"  mret\n"                  // Return from machine trap to after_ecall.
"trap_fail:\n"              // Trap failure path used for incorrect mcause values.
"  li a0, 2\n"              // Report a distinct failure code for bad trap metadata.
"  ebreak\n"                // Stop the emulator with the failure code in a0.
);
