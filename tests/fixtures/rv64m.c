typedef unsigned long u64;
typedef unsigned char u8;

#define UART ((volatile u8 *)0x10000000UL)
#define UART_LSR_THRE 0x20

static void uart_putc(u8 value) {
    while ((UART[5] & UART_LSR_THRE) == 0) {
    }
    UART[0] = value;
}

static volatile u64 lhs64 = 123456789UL;
static volatile u64 rhs64 = 37;
static volatile unsigned lhs32 = 100000U;
static volatile unsigned rhs32 = 3;
static volatile u64 sink;

__attribute__((noreturn)) void _start(void) {
    u64 a = lhs64;
    u64 b = rhs64;
    unsigned c = lhs32;
    unsigned d = rhs32;

    u64 product = a * b;
    u64 quotient = product / b;
    u64 remainder = product % b;
    unsigned word_product = c * d;
    unsigned word_quotient = word_product / d;
    unsigned word_remainder = word_product % d;

    sink = product ^ quotient ^ remainder ^ word_product ^ word_quotient ^ word_remainder;

    u64 code = 0;
    if (quotient != a) {
        code |= 1;
    }
    if (remainder != 0) {
        code |= 2;
    }
    if (word_quotient != c) {
        code |= 4;
    }
    if (word_remainder != 0) {
        code |= 8;
    }

    uart_putc((u8)'M');
    __asm__ volatile("mv a0, %0\n\tebreak" : : "r"(code) : "a0");
    __builtin_unreachable();
}
