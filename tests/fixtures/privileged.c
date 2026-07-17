typedef unsigned long u64;
typedef unsigned char u8;

#define PHYS_BASE 0x80000000UL
#define VIRT_BASE 0x40000000UL
#define VIRT_USER 0x60000000UL
#define UART_BASE 0x10000000UL
#define PAGE_SHIFT 12
#define PTE_V (1UL << 0)
#define PTE_R (1UL << 1)
#define PTE_W (1UL << 2)
#define PTE_X (1UL << 3)
#define PTE_U (1UL << 4)
#define PTE_A (1UL << 6)
#define PTE_D (1UL << 7)
#define SATP_SV39 (8UL << 60)
#define MSTATUS_MPP_MASK (3UL << 11)
#define MSTATUS_MPP_SUPERVISOR (1UL << 11)
#define MSTATUS_MPRV (1UL << 17)
#define MSTATUS_SUM (1UL << 18)
#define MSTATUS_MXR (1UL << 19)

static u64 root[512] __attribute__((aligned(4096)));
static u64 level1_code[512] __attribute__((aligned(4096)));
static u64 level1_uart[512] __attribute__((aligned(4096)));
static u64 level0_user[512] __attribute__((aligned(4096)));
static u64 user_page[512] __attribute__((aligned(4096)));

static u64 vpn2(u64 address) { return (address >> 30) & 0x1ff; }
static u64 vpn1(u64 address) { return (address >> 21) & 0x1ff; }
static u64 vpn0(u64 address) { return (address >> 12) & 0x1ff; }
static u64 pte(u64 physical, u64 flags) {
    return ((physical >> PAGE_SHIFT) << 10) | flags;
}
static u64 virt(u64 physical) { return physical - PHYS_BASE + VIRT_BASE; }

#define READ_CSR(name) ({ u64 value; __asm__ volatile("csrr %0, " #name : "=r"(value)); value; })
#define WRITE_CSR(name, value) __asm__ volatile("csrw " #name ", %0" : : "r"((u64)(value)) : "memory")

__attribute__((noreturn)) static void finish(u64 code) {
    volatile u8 *uart = (volatile u8 *)UART_BASE;
    uart[0] = code == 0 ? 'P' : 'F';
    __asm__ volatile("mv a0, %0\n\tebreak" : : "r"(code) : "a0");
    __builtin_unreachable();
}

__attribute__((noreturn)) static void supervisor_entry(void) {
    u64 code = 0;
    if (READ_CSR(time) == 0) code |= 4;
    __asm__ volatile("sfence.vma zero, zero" ::: "memory");
    if (*(volatile u64 *)VIRT_USER != 0x50524956UL) code |= 8;
    finish(code);
}

__attribute__((noreturn)) void _start(void) {
    root[vpn2(VIRT_BASE)] = pte((u64)level1_code, PTE_V);
    level1_code[vpn1(VIRT_BASE)] =
        pte(PHYS_BASE, PTE_V | PTE_R | PTE_W | PTE_X | PTE_A | PTE_D);
    level1_code[vpn1(VIRT_USER)] = pte((u64)level0_user, PTE_V);
    level0_user[vpn0(VIRT_USER)] =
        pte((u64)user_page, PTE_V | PTE_X | PTE_U | PTE_A);

    root[vpn2(UART_BASE)] = pte((u64)level1_uart, PTE_V);
    level1_uart[vpn1(UART_BASE)] =
        pte(UART_BASE, PTE_V | PTE_R | PTE_W | PTE_A | PTE_D);
    user_page[0] = 0x50524956UL;

    WRITE_CSR(mcounteren, 7);
    WRITE_CSR(scounteren, 7);
    WRITE_CSR(satp, SATP_SV39 | ((u64)root >> PAGE_SHIFT));
    __asm__ volatile("sfence.vma zero, zero" ::: "memory");

    u64 status = (READ_CSR(mstatus) & ~MSTATUS_MPP_MASK) |
        MSTATUS_MPP_SUPERVISOR | MSTATUS_MPRV | MSTATUS_SUM | MSTATUS_MXR;
    WRITE_CSR(mstatus, status);
    if (*(volatile u64 *)VIRT_USER != 0x50524956UL) finish(1);

    WRITE_CSR(mepc, virt((u64)supervisor_entry));
    WRITE_CSR(mstatus, status & ~MSTATUS_MPRV);
    __asm__ volatile("mret");
    __builtin_unreachable();
}
