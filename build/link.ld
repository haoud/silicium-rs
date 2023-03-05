ENTRY(_start)
OUTPUT_FORMAT("elf64-x86-64")

KERNEL_VIRT_BASE = 0xFFFFFFFF80000000;

SECTIONS
{
    . = KERNEL_VIRT_BASE;
    . = ALIGN(4096);
    .text :
    {
        *(.text*)
    }

    . = ALIGN(4096);
    .data :
    {
        *(.data*)
    }

    . = ALIGN(4096);
    .rodata :
    {
        *(.rodata*)
    }

    . = ALIGN(4096);
    .per_cpu :
    {
        __per_cpu_start = .;
        *(.tdata*)
        *(.tbss*)
        . = ALIGN(8);
        __per_cpu_end = .;
    }

    . = ALIGN(4096);
    .bss (NOLOAD) :
    {
        *(.bss*)
    }

    . = ALIGN(4096);
    _end = .;
    
    /DISCARD/ :
    {
        *(.fini_array*)
        *(.eh_frame*)
        *(.comment)
        *(note*)
    }
}
