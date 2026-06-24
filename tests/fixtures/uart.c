typedef unsigned char u8;

typedef unsigned long usize;

#define UART ((volatile u8 *)0x10000000UL)
#define UART_LSR_DATA_READY 0x01
#define UART_LSR_THRE 0x20

static void uart_putc(u8 value) {
    while ((UART[5] & UART_LSR_THRE) == 0) {
    }
    UART[0] = value;
}

static u8 uart_getc(void) {
    while ((UART[5] & UART_LSR_DATA_READY) == 0) {
    }
    return UART[0];
}

static void uart_puts(const char *value) {
    while (*value != 0) {
        uart_putc((u8)*value);
        ++value;
    }
}

__attribute__((noreturn)) void _start(void) {
    char name[32];
    usize length = 0;

    uart_puts("name?\n");
    for (;;) {
        u8 value = uart_getc();
        if (value == (u8)10) {
            break;
        }
        if (length + 1 < sizeof(name)) {
            name[length] = (char)value;
            ++length;
        }
    }
    name[length] = 0;

    uart_puts("oh hai ");
    uart_puts(name);
    uart_putc((u8)33);
    uart_putc((u8)10);

    __asm__ volatile("li a0, 0\n\tebreak" ::: "a0");
    __builtin_unreachable();
}
