typedef unsigned char u8;

#define UART ((volatile u8 *)0x10000000UL)
#define UART_LSR_DATA_READY 0x01
#define UART_LSR_THRE 0x20

static void uart_putc(u8 value) {
    while ((UART[5] & UART_LSR_THRE) == 0) {}
    UART[0] = value;
}

static u8 uart_getc(void) {
    while ((UART[5] & UART_LSR_DATA_READY) == 0) {
        __asm__ volatile("wfi");
    }
    return UART[0];
}

static void uart_puts(const char *text) {
    while (*text) uart_putc((u8)*text++);
}

__attribute__((noreturn)) void _start(void) {
    u8 line[128];
    unsigned long length = 0;
    uart_puts("uart echo ready\n");
    for (;;) {
        u8 byte = uart_getc();
        if (byte == '\r') continue;
        if (byte == '\n') {
            uart_puts("got: ");
            for (unsigned long index = 0; index < length; ++index) uart_putc(line[index]);
            uart_putc('\n');
            length = 0;
        } else if (length < sizeof(line)) {
            line[length++] = byte;
        }
    }
}
