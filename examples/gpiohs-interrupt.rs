#![no_std]
#![no_main]

use k210_hal::{prelude::*, pac, plic::*, fpioa, gpiohs::Edge, stdout::Stdout};
use panic_halt as _;
use riscv::register::{mie,mstatus,mhartid,mcause};
use core::sync::atomic::{AtomicBool, Ordering};

static INTR: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Copy, Clone)]
struct IntrInfo {
    hart_id: usize,
    cause: usize,
}

static mut INTR_INFO: Option<IntrInfo> = None;

#[export_name = "MachineExternal"]
fn my_trap_handler() {
    let hart_id = mhartid::read();
    let threshold = pac::PLIC::get_threshold(hart_id);

    let irq = pac::PLIC::claim(hart_id).unwrap();
    let prio = pac::PLIC::get_priority(irq);
    unsafe { 
        pac::PLIC::set_threshold(hart_id, prio);
        mie::clear_msoft();
        mie::clear_mtimer();
    }

    unsafe { 
        &(*pac::GPIOHS::ptr()).rise_ie.write(|w| w.pin0().clear_bit());
        &(*pac::GPIOHS::ptr()).rise_ip.write(|w| w.pin0().set_bit());
        &(*pac::GPIOHS::ptr()).rise_ie.write(|w| w.pin0().set_bit());
    
        &(*pac::GPIOHS::ptr()).fall_ie.write(|w| w.pin0().clear_bit());
        &(*pac::GPIOHS::ptr()).fall_ip.write(|w| w.pin0().set_bit());
        &(*pac::GPIOHS::ptr()).fall_ie.write(|w| w.pin0().set_bit());
    }

    // actual handle process starts
    let stdout = unsafe { &mut *SHARED_STDOUT.as_mut_ptr() };
    let cause = mcause::read().bits();

    writeln!(stdout, "Interrupt!!! {} {:016X}", hart_id, cause).unwrap();

    unsafe { INTR_INFO = Some(IntrInfo { hart_id, cause }); }

    INTR.store(true, Ordering::SeqCst);
    // actual handle process ends

    unsafe { 
        mie::set_msoft();
        mie::set_mtimer();
        pac::PLIC::set_threshold(hart_id, threshold);
    }
    pac::PLIC::complete(hart_id, irq);
}

static mut SHARED_STDOUT: core::mem::MaybeUninit<
    k210_hal::stdout::Stdout<k210_hal::serial::Tx<pac::UARTHS>>
> = core::mem::MaybeUninit::uninit();

#[riscv_rt::entry]
fn main() -> ! {
    let hart_id = mhartid::read();

    let p = pac::Peripherals::take().unwrap();

    let mut sysctl = p.SYSCTL.constrain();
    let fpioa = p.FPIOA.split(&mut sysctl.apb0);
    let gpiohs = p.GPIOHS.split();
    fpioa.io16.into_function(fpioa::GPIOHS0);
    let mut boot = gpiohs.gpiohs0.into_pull_up_input();

    // Configure clocks (TODO)
    let clocks = k210_hal::clock::Clocks::new();

    // Configure UART
    let serial = p.UARTHS.configure(115_200.bps(), &clocks);
    let (mut tx, _) = serial.split();

    let mut stdout = Stdout(&mut tx);

    writeln!(stdout, "This code is running on hart {}", mhartid::read()).unwrap();

    writeln!(stdout, "Initializing interrupts").unwrap();
    unsafe {
        // set PLIC threshold for current core
        pac::PLIC::set_threshold(hart_id, Priority::P0);
        // Enable interrupts in general
        mstatus::set_mie();
        // Set the Machine-External bit in MIE
        mie::set_mext();
    }
    
    writeln!(stdout, "Enabling interrupt trigger for GPIOHS0").unwrap();
    boot.trigger_on_edge(Edge::RISING | Edge::FALLING);

    // enable IRQ for gpiohs0 interrupt 
    writeln!(stdout, "Enabling IRQ for GPIOHS0").unwrap();
    unsafe {
        // set_priority
        pac::PLIC::set_priority(Interrupt::GPIOHS0, Priority::P1);
        // mask
        pac::PLIC::enable(hart_id, Interrupt::GPIOHS0);
    }

    // verify irq write 
    // for irq_number in 1..=65 {
    //     let enabled = unsafe {
    //         &(*pac::PLIC::ptr()).target_enables[hart_id].enable[irq_number / 32]
    //             .read().bits() & (1 << (irq_number % 32)) != 0
    //     };
    //     if !enabled { 
    //         continue;
    //     }
    //     let priority = unsafe {
    //         &(*pac::PLIC::ptr()).priority[irq_number].read().bits()
    //     };
    //     writeln!(stdout, 
    //         "Irq: {}; Enabled: {}; Priority: {}", 
    //         irq_number, enabled, priority
    //     ).ok();
    // }

    // writeln!(stdout, "Generate IPI for core {} !", hart_id).unwrap();
    // msip::set_value(hart_id, true);

    writeln!(stdout, "Configuration finished!").unwrap();

    loop { 
        writeln!(stdout, "Waiting for interrupt").unwrap();
        unsafe { riscv::asm::wfi(); } 

        while !INTR.load(Ordering::SeqCst) {
            use core::sync::atomic::{self, Ordering};
            atomic::compiler_fence(Ordering::SeqCst);
        }
        INTR.store(false, Ordering::SeqCst);

        writeln!(stdout, 
            "Interrupt was triggered! hart_id: {:16X}, cause: {:16X}", 
            unsafe { INTR_INFO }.unwrap().hart_id,
            unsafe { INTR_INFO }.unwrap().cause,
        ).unwrap();
    }
}
