typedef unsigned long u64;
typedef unsigned char u8;

#define PHYS_BASE 0x80000000UL
#define VIRT_BASE 0x40000000UL
#define PHYS_VALUE (PHYS_BASE + 0x5000UL)
#define VIRT_VALUE (VIRT_BASE + 0x5000UL)
#define UART_BASE 0x10000000UL
#define PAGE_SHIFT 12
#define PTE_V (1UL << 0)
#define PTE_R (1UL << 1)
#define PTE_W (1UL << 2)
#define PTE_X (1UL << 3)
#define PTE_A (1UL << 6)
#define PTE_D (1UL << 7)
#define SATP_SV39 (8UL << 60)
#define MSTATUS_MPP_MASK (3UL << 11)
#define MSTATUS_MPP_SUPERVISOR (1UL << 11)

static u64 root[512] __attribute__((aligned(4096)));
static u64 level1_code[512] __attribute__((aligned(4096)));
static u64 level1_uart[512] __attribute__((aligned(4096)));

static u64 vpn2(u64 address) {
    return (address >> 30) & 0x1ff;
}

static u64 vpn1(u64 address) {
    return (address >> 21) & 0x1ff;
}

static u64 pte(u64 physical, u64 flags) {
    return ((physical >> PAGE_SHIFT) << 10) | flags;
}

static u64 virt(u64 physical) {
    return physical - PHYS_BASE + VIRT_BASE;
}

static void write_csr_satp(u64 value) {
    __asm__ volatile("csrw satp, %0" : : "r"(value) : "memory");
}

static void write_csr_mepc(u64 value) {
    __asm__ volatile("csrw mepc, %0" : : "r"(value) : "memory");
}

static u64 read_csr_mstatus(void) {
    u64 value;
    __asm__ volatile("csrr %0, mstatus" : "=r"(value));
    return value;
}

static void write_csr_mstatus(u64 value) {
    __asm__ volatile("csrw mstatus, %0" : : "r"(value) : "memory");
}

__attribute__((noreturn)) static void supervisor_entry(void) {
    volatile u64 *value = (volatile u64 *)VIRT_VALUE;
    volatile u8 *uart = (volatile u8 *)UART_BASE;
    uart[0] = (u8)*value;
    __asm__ volatile("li a0, 0\n\tebreak" ::: "a0");
    __builtin_unreachable();
}

__attribute__((noreturn)) void _start(void) {
    root[vpn2(VIRT_BASE)] = pte((u64)level1_code, PTE_V);
    level1_code[vpn1(VIRT_BASE)] = pte(PHYS_BASE, PTE_V | PTE_R | PTE_W | PTE_X | PTE_A | PTE_D);

    root[vpn2(UART_BASE)] = pte((u64)level1_uart, PTE_V);
    level1_uart[vpn1(UART_BASE)] = pte(UART_BASE, PTE_V | PTE_R | PTE_W | PTE_A | PTE_D);
    *(volatile u64 *)PHYS_VALUE = 0x56UL;

    write_csr_satp(SATP_SV39 | ((u64)root >> PAGE_SHIFT));
    write_csr_mepc(virt((u64)supervisor_entry));
    write_csr_mstatus((read_csr_mstatus() & ~MSTATUS_MPP_MASK) | MSTATUS_MPP_SUPERVISOR);
    __asm__ volatile("mret");
    __builtin_unreachable();
}
