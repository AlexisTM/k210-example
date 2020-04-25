#![allow(warnings)]
#![no_std]
#![no_main]

use panic_halt as _;
use k210_hal::prelude::*;
use k210_hal::fpioa;
use k210_hal::pac as pac;
use k210_hal::stdout::Stdout;

#[riscv_rt::entry]
fn main() -> ! {
    let p = pac::Peripherals::take().unwrap();

    // Prepare pins for UARTHS
    let fpioa = p.FPIOA.split();
    // let tx = p.pins.pin5;
    let _io5 = fpioa.io5.into_function(fpioa::UarthsTx);
    // let rx = p.pins.pin4;

    // Configure clocks (TODO)
    let clocks = k210_hal::clock::Clocks::new();

    // Configure UART
    let serial = p.UARTHS.configure(
        115_200.bps(), 
        &clocks
    );
    let (mut tx, _) = serial.split();

    // todo: new stdout design (simple Write impl?)
    let mut stdout = Stdout(&mut tx);

    writeln!(stdout, "Hello, Rust!").unwrap();

    loop {
        writeln!(stdout, "Rust NB!").unwrap();
    }
}
