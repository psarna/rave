typedef unsigned char u8;

#define UART ((volatile u8 *)0x10000000UL)
#define UART_LSR_THRE 0x20

static void uart_putc(u8 value) {
    while ((UART[5] & UART_LSR_THRE) == 0) {}
    UART[0] = value;
}

__attribute__((noreturn)) void _start(void) {
    uart_putc('B');
    uart_putc('\n');
    for (;;) __asm__ volatile("wfi");
}
