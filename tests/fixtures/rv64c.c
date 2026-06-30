typedef unsigned long u64;
typedef unsigned char u8;

#define UART ((volatile u8 *)0x10000000UL)
#define UART_LSR_THRE 0x20

static void uart_putc(u8 value) {
    while ((UART[5] & UART_LSR_THRE) == 0) {
    }
    UART[0] = value;
}

__attribute__((noreturn)) void _start(void) {
    register u64 a asm("s0") = 7;
    register u64 b asm("s1") = 5;
    u64 value = 0;
    value += a;
    value -= b;
    value += 1;
    if (value == 3) {
        uart_putc('C');
    }
    u64 code = value != 3;
    __asm__ volatile("mv a0, %0\n\tebreak" : : "r"(code) : "a0");
    __builtin_unreachable();
}
