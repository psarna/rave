typedef unsigned long u64;
typedef unsigned char u8;

#define UART ((volatile u8 *)0x10000000UL)
#define UART_LSR_THRE 0x20

static void uart_putc(u8 value) {
    while ((UART[5] & UART_LSR_THRE) == 0) {
    }
    UART[0] = value;
}

static volatile const u64 input[] = {3, 1, 4, 1, 5};
static volatile u64 output;

__attribute__((noreturn)) void _start(void) {
    u64 sum = 0;
    for (u64 i = 0; i < sizeof(input) / sizeof(input[0]); ++i) {
        sum += input[i];
    }
    output = sum;
    uart_putc((u8)('A' + sum));
    u64 code = sum != 14;
    __asm__ volatile("mv a0, %0\n\tebreak" : : "r"(code) : "a0");
    __builtin_unreachable();
}
