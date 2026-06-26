typedef unsigned long u64;
typedef unsigned char u8;

#define UART ((volatile u8 *)0x10000000UL)
#define UART_LSR_THRE 0x20

static void uart_putc(u8 value) {
    while ((UART[5] & UART_LSR_THRE) == 0) {
    }
    UART[0] = value;
}

#define READ_CSR(name) ({ \
    u64 value; \
    __asm__ volatile("csrr %0, " #name : "=r"(value)); \
    value; \
})

#define WRITE_CSR(name, value) \
    __asm__ volatile("csrw " #name ", %0" : : "r"((u64)(value)) : "memory")

#define SET_CSR(name, value) \
    __asm__ volatile("csrs " #name ", %0" : : "r"((u64)(value)) : "memory")

#define CLEAR_CSR(name, value) \
    __asm__ volatile("csrc " #name ", %0" : : "r"((u64)(value)) : "memory")

__attribute__((noreturn)) void _start(void) {
    u64 code = 0;

    WRITE_CSR(mscratch, 0x1200UL);
    SET_CSR(mscratch, 0x0034UL);
    CLEAR_CSR(mscratch, 0x0200UL);
    if (READ_CSR(mscratch) != 0x1034UL) {
        code |= 1;
    }

    WRITE_CSR(mstatus, ~0UL);
    if (READ_CSR(mstatus) != ((1UL << 3) | (1UL << 7) | (3UL << 11))) {
        code |= 2;
    }

    WRITE_CSR(mepc, ~0UL);
    if ((READ_CSR(mepc) & 1UL) != 0) {
        code |= 4;
    }

    u64 misa = READ_CSR(misa);
    if (((misa >> 62) != 2) || ((misa & (1UL << 8)) == 0) || ((misa & (1UL << 12)) == 0)) {
        code |= 8;
    }

    u64 before = READ_CSR(cycle);
    __asm__ volatile("addi zero, zero, 0\n\taddi zero, zero, 0" ::: "memory");
    u64 after = READ_CSR(cycle);
    if (after <= before) {
        code |= 16;
    }

    if (READ_CSR(instret) == 0) {
        code |= 32;
    }

    uart_putc((u8)'C');
    __asm__ volatile("mv a0, %0\n\tebreak" : : "r"(code) : "a0");
    __builtin_unreachable();
}
