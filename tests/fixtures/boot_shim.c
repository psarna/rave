__asm__(
".section .text\n"
".globl _start\n"
"_start:\n"
"  li t0, 0x80200000\n"
"  csrw mepc, t0\n"
"  csrr t0, mstatus\n"
"  li t1, ~(3 << 11)\n"
"  and t0, t0, t1\n"
"  li t1, (1 << 11)\n"
"  or t0, t0, t1\n"
"  csrw mstatus, t0\n"
"  mret\n"
);
