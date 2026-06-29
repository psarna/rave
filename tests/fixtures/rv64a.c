typedef unsigned long u64;
typedef unsigned char u8;

#define UART ((volatile u8 *)0x10000000UL)
#define UART_LSR_THRE 0x20

static void uart_putc(u8 value) {
    while ((UART[5] & UART_LSR_THRE) == 0) {
    }
    UART[0] = value;
}

static volatile u64 counter = 41;
static volatile unsigned word_counter = 0xfffffff0U;
static volatile u64 lock = 0;
static volatile u64 sink;

__attribute__((noreturn)) void _start(void) {
    u64 code = 0;

    u64 old = __atomic_fetch_add(&counter, 1, __ATOMIC_SEQ_CST);
    if (old != 41 || counter != 42) {
        code |= 1;
    }

    unsigned word_old = __atomic_fetch_xor(&word_counter, 0x0fU, __ATOMIC_SEQ_CST);
    if (word_old != 0xfffffff0U || word_counter != 0xffffffffU) {
        code |= 2;
    }

    u64 expected = 0;
    if (!__atomic_compare_exchange_n(&lock, &expected, 1, 0, __ATOMIC_SEQ_CST, __ATOMIC_SEQ_CST)) {
        code |= 4;
    }
    if (expected != 0 || lock != 1) {
        code |= 8;
    }

    expected = 0;
    if (__atomic_compare_exchange_n(&lock, &expected, 2, 0, __ATOMIC_SEQ_CST, __ATOMIC_SEQ_CST)) {
        code |= 16;
    }
    if (expected != 1 || lock != 1) {
        code |= 32;
    }

    sink = counter ^ word_counter ^ lock ^ expected;
    uart_putc((u8)'A');
    __asm__ volatile("mv a0, %0\n\tebreak" : : "r"(code) : "a0");
    __builtin_unreachable();
}
