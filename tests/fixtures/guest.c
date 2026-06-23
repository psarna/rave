typedef unsigned long u64;

static volatile const u64 input[] = {3, 1, 4, 1, 5};
static volatile u64 output;

__attribute__((noreturn)) void _start(void) {
    u64 sum = 0;
    for (u64 i = 0; i < sizeof(input) / sizeof(input[0]); ++i) {
        sum += input[i];
    }
    output = sum;
    u64 code = sum != 14;
    __asm__ volatile("mv a0, %0\n\tebreak" : : "r"(code) : "a0");
    __builtin_unreachable();
}
