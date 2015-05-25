use core::prelude::*;
use core::intrinsics;
use hil::{uart, Controller};

use nvic;
use pm::{self, Clock, PBAClock};

#[repr(C, packed)]
struct UsartRegisters {
    cr: u32,
    mr: u32,
    ier: u32,
    idr: u32,
    imr: u32,
    csr: u32,
    rhr: u32,
    thr: u32,
    brgr: u32, // 0x20
    rtor: u32,
    ttgr: u32,
    reserved0: [u32; 5],
    fidi: u32, // 0x40
    ner: u32,
    reserved1: u32,
    ifr: u32,
    man: u32,
    linmr: u32,
    linir: u32,
    linbrr: u32,
    wpmr: u32,
    wpsr: u32,
    version: u32
}

const SIZE: usize = 0x4000;
const BASE_ADDRESS: usize = 0x40024000;

#[derive(Copy,Clone)]
pub enum Location {
    USART0, USART1, USART2, USART3
}

pub struct USART {
    regs: &'static mut UsartRegisters,
    client: Option<&'static mut uart::Reader>,
    clock: Clock,
    nvic: nvic::NvicIdx,
}

impl Controller for USART {
    type Config = Option<&'static mut uart::Reader>;

    fn configure(&mut self, client: Option<&'static mut uart::Reader>) {
        self.client = client;
    }
}

impl USART {
    pub fn new(location: Location) -> USART {
        let address = BASE_ADDRESS + (location as usize) * SIZE;

        let pba_clock = match location {
            Location::USART0 => PBAClock::USART0,
            Location::USART1 => PBAClock::USART1,
            Location::USART2 => PBAClock::USART2,
            Location::USART3 => PBAClock::USART3,
        };

        let nvic = match location {
            Location::USART0 => nvic::NvicIdx::USART0,
            Location::USART1 => nvic::NvicIdx::USART1,
            Location::USART2 => nvic::NvicIdx::USART2,
            Location::USART3 => nvic::NvicIdx::USART3
        };


        USART {
            regs: unsafe { intrinsics::transmute(address) },
            clock: Clock::PBA(pba_clock),
            nvic: nvic,
            client: None
        }
    }

    fn set_baud_rate(&mut self, baud_rate: u32) {
        let cd = 48000000 / (16 * baud_rate);
        volatile!(self.regs.brgr = cd);
    }

    // This can be made safe by having a struct represent the mode register,
    // with enums when there are choices and not just numbers, and passing the
    // struct to this function. As is, it's too easy to make a mistake.
    unsafe fn set_mode(&mut self, mode: u32) {
        #![allow(unused_unsafe)]
        volatile!(self.regs.mr = mode);
    }

    fn enable_clock(&self) {
        unsafe {
            pm::enable_clock(self.clock);
        }
    }

    pub fn rx_ready(&self) -> bool {
        volatile!(self.regs.csr) & 0b1 != 0
    }

    pub fn tx_ready(&self) -> bool {
        volatile!(self.regs.csr) & 0b10 != 0
    }

    fn enable_nvic(&self) {
        unsafe {
            nvic::enable(self.nvic);
        }
    }

    fn disable_nvic(&self) {
        unsafe {
            nvic::disable(self.nvic);
        }
    }

    pub fn enable_rx_interrupts(&mut self) {
        self.enable_nvic();
        volatile!(self.regs.ier = 1 as u32);
    }

    pub fn disable_rx_interrupts(&mut self) {
        self.disable_nvic();
        volatile!(self.regs.idr = 1 as u32);
    }

    pub fn interrupt_fired(&mut self) {
        if self.rx_ready() {
            let c = volatile!(self.regs.rhr) as u8;
            match self.client {
                Some(ref mut client) => {client.read_done(c)},
                None => {}
            }
        }
    }

    pub fn reset_rx(&mut self) {
        volatile!(self.regs.cr = 1 << 2);
    }

}

impl uart::UART for USART {
    fn init(&mut self, params: uart::UARTParams) {
        let chrl = ((params.data_bits - 1) & 0x3) as u32;
        let mode = 0 /* mode */
            | 0 << 4 /*USCLKS*/
            | chrl << 6 /* Character Length */
            | (params.parity as u32) << 9 /* Parity */
            | 0 << 12; /* Number of stop bits = 1 */;

        self.enable_clock();
        self.set_baud_rate(params.baud_rate);
        unsafe { self.set_mode(mode); }
        volatile!(self.regs.ttgr = 4);
    }

    fn send_byte(&mut self, byte: u8) {
        while !self.tx_ready() {}
        volatile!(self.regs.thr = byte as u32);
    }

    fn read_byte(&self) -> u8 {
        while !self.rx_ready() {}
        unsafe {
            intrinsics::volatile_load(&self.regs.rhr) as u8
        }
    }

    fn enable_rx(&mut self) {
        volatile!(self.regs.cr = 1 << 4);
    }

    fn disable_rx(&mut self) {
        volatile!(self.regs.cr = 1 << 5);
    }

    fn enable_tx(&mut self) {
        volatile!(self.regs.cr = 1 << 6);
    }

    fn disable_tx(&mut self) {
        volatile!(self.regs.cr = 1 << 7);
    }

}

