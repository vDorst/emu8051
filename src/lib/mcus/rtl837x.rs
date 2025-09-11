use std::ops::Neg;

use log::{debug, error};
use tokio::sync::mpsc::{Receiver, Sender};

use crate::traits::component::MCU;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MCS51_REGISTERS {
    SFR_FLASH_EXEC = 0,
    SP,
    DPL1,
    DPH1,
    DPL2,
    DPH2,
    DPS,
    PCON,
    TCON,
    TMOD,
    TL0,
    TL1,
    TH0,
    TH1,
    P1,
    SCON,
    SBUF,
    SFR_EXEC_GO,
    IE,
    P3,
    IP,
    T2CON,
    RCAP2L,
    RCAP2H,
    TL2,
    TH2,
    PSW,
    ACC,
    B,
    SFR_EXEC_STATUS,
    SFR_REG_ADDRH,
    SFR_REG_ADDRL,
    SFR_REG_DATA_24,
    SFR_REG_DATA_16,
    SFR_REG_DATA_8,
    SFR_REG_DATA_0,
    SFR_SMI_REGH,
    SFR_SMI_REGL,
    SFR_SMI_DEV,
    SFR_SMI_PHYMASK,
    EXIF,
    EIE,
    PSBANK,
    SFR_FLASH_CMD_R,
    SFR_FLASH_CONFIG,
    SFR_FLASH_CMD,
    REGISTER_COUNT,
}

#[derive(Debug, Clone, Copy)]
pub enum MCS51_ADDRESSING {
    ACCUMULATOR,
    REGISTER(u8),
    DIRECT(u8),
    INDIRECT_Ri(u8),
    DATA(u8),
    ADDR_16,
    ADDR_11,
    RELATIVE,
}

pub struct DW8051_RTL837x {
    pub pc: u16,
    pub op_pc: u16,
    pub interrupts: i16,
    program: Vec<u8>,
    pub special_function_registers: [u8; MCS51_REGISTERS::REGISTER_COUNT as usize],
    pub ram: [u8; 256],
    pub xdata: [u8; 48 * 1024],
    pub additional_cycles: u8,
    pub dispatch: [fn(&mut DW8051_RTL837x); 256],
    pub debug: bool,
    pub ext_flash: Option<(Sender<u8>, Receiver<[u8; 4]>)>,
    pub ext_uart_0: Option<(Sender<u8>, Receiver<u8>)>,
    pub ext_asic: Option<(Sender<u8>, Receiver<[u8; 4]>)>,
    pub ext_nic: Option<(Sender<u8>, Receiver<[u8; 4]>)>,
}

impl DW8051_RTL837x {
    pub fn new() -> DW8051_RTL837x {
        DW8051_RTL837x {
            pc: 0,
            op_pc: 0,
            interrupts: 0,
            ram: [0; 256],
            program: vec![],
            special_function_registers: [0; MCS51_REGISTERS::REGISTER_COUNT as usize],
            additional_cycles: 0,
            dispatch: [|_cpu| {}; 256],
            debug: false,
            xdata: [0; _],
            ext_flash: None,
            ext_uart_0: None,
            ext_asic: None,
            ext_nic: None,
        }
    }

    pub fn push_stack(&mut self, value: u8) {
        self.write_sfr_rel(MCS51_REGISTERS::SP, 1, false);
        let sp = self.get_stack_pointer();
        self.write(sp, value);
    }

    pub fn pop_stack(&mut self) -> u8 {
        let sp = self.get_stack_pointer();
        let val = self.read_idirect(sp).unwrap();
        self.write_sfr_rel(MCS51_REGISTERS::SP, 1, true);
        val
    }

    pub fn set_stack_pointer(&mut self, value: u8) {
        self.write_sfr(MCS51_REGISTERS::SP, value);
    }

    pub fn get_stack_pointer(&mut self) -> u8 {
        self.read_sfr(MCS51_REGISTERS::SP)
    }

    pub fn get_sfr_mut(&mut self, register: MCS51_REGISTERS) -> Option<&mut u8> {
        self.special_function_registers.get_mut(register as usize)
    }

    pub fn read_sfr(&self, register: MCS51_REGISTERS) -> u8 {
        self.special_function_registers[register as usize]
    }

    pub fn write_sfr(&mut self, register: MCS51_REGISTERS, value: u8) {
        if register == MCS51_REGISTERS::SBUF {
            println!("SBUF = {value}");
        }
        self.special_function_registers[register as usize] = value;
    }

    pub fn write_sfr_rel(&mut self, register: MCS51_REGISTERS, value: u8, sub: bool) {
        if register == MCS51_REGISTERS::SBUF {
            println!("SBUF = {value}");
        }
        if sub {
            self.special_function_registers[register as usize] =
                self.special_function_registers[register as usize].wrapping_sub(value);
        } else {
            self.special_function_registers[register as usize] =
                self.special_function_registers[register as usize].wrapping_add(value);
        };
    }

    pub fn write_pc_rel(&mut self, value: u16, sub: bool) {
        if sub {
            self.pc = self.pc.wrapping_sub(value);
        } else {
            self.pc = self.pc.wrapping_add(value);
        }
    }

    pub fn write_pc_reli(&mut self, value: i16) {
        self.pc = self.pc.wrapping_add_signed(value);
    }

    pub fn get_current_register_bank(&self) -> u8 {
        self.get_current_register_bank_flags() >> 3
    }

    pub fn get_current_register_bank_flags(&self) -> u8 {
        let psw = self.read_sfr(MCS51_REGISTERS::PSW);

        psw & 0b11000
    }

    pub fn get_register_mut(&mut self, register: u8) -> Option<&mut u8> {
        if register == 99 {
            println!("SBUS");
        }
        let bank = self.get_current_register_bank_flags();
        self.ram.get_mut(register as usize + bank as usize)
    }

    pub fn read_register(&self, register: u8) -> u8 {
        let bank = self.get_current_register_bank_flags();
        self.ram[register as usize + bank as usize]
    }

    pub fn write_register(&mut self, register: u8, value: u8) {
        let bank = self.get_current_register_bank_flags();

        if register == 99 {
            println!("SBUS = {value}");
        }

        self.ram[register as usize + bank as usize] = value;
    }

    pub fn read_code_byte(&mut self, addr: usize) -> u8 {
        let op = self.program[addr];
        print!("{op:02x}");
        op
    }

    /*
    Bit Addressable Area: 16 bytes have been assigned for this segment, 20H-2FH. Each one of the 128 bits of this
    segment can be directly addressed (0-7FH).
    The bits can be referred to in two ways both of which are acceptable by the ASM-51. One way is to refer to their
    addresses, ie. 0 to 7FH. The other way is with reference to bytes 20H to 2FH. Thus, bits 0–7 can also be referred to
    as bits 20.0–20.7, and bits 8-FH are the same as 21.0–21.7 and so on.
    Each of the 16 bytes in this segment can also be addressed as a byte.
    */

    pub fn read_bit(&self, address: u8) -> bool {
        let register = address & 0xF8;
        let bit = 1 << (address & 0x7);

        let value = self.read(register);
        let val = *value.unwrap();
        (val & bit) != 0
    }

    pub fn write_bit(&mut self, address: u8, value: bool) {
        let addr = address & 0xF8;

        // println!("W-BIT {:0x} {:0x} {value}", address, addr);

        let bit = address & 0x7;
        let mut src = *self.read(addr).unwrap();

        if value {
            src |= (value as u8) << bit;
        } else {
            src &= ((!value) as u8) << bit;
        }

        self.write(addr, src);
    }

    pub fn read_raw(&self, address: u8) -> u8 {
        match address {
            0x00..=0x7F => self.ram[address as usize],
            0x80 => self.special_function_registers[MCS51_REGISTERS::SFR_FLASH_EXEC as usize],
            0x81 => self.special_function_registers[MCS51_REGISTERS::SP as usize],
            0x82 => self.special_function_registers[MCS51_REGISTERS::DPL1 as usize],
            0x83 => self.special_function_registers[MCS51_REGISTERS::DPH1 as usize],
            0x84 => self.special_function_registers[MCS51_REGISTERS::DPL2 as usize],
            0x85 => self.special_function_registers[MCS51_REGISTERS::DPH2 as usize],
            0x86 => self.special_function_registers[MCS51_REGISTERS::DPS as usize],
            0x87 => self.special_function_registers[MCS51_REGISTERS::PCON as usize],
            0x88 => self.special_function_registers[MCS51_REGISTERS::TCON as usize],
            0x89 => self.special_function_registers[MCS51_REGISTERS::TMOD as usize],
            0x8A => self.special_function_registers[MCS51_REGISTERS::TL0 as usize],
            0x8B => self.special_function_registers[MCS51_REGISTERS::TL1 as usize],
            0x8C => self.special_function_registers[MCS51_REGISTERS::TH0 as usize],
            0x8D => self.special_function_registers[MCS51_REGISTERS::TH1 as usize],
            0x90 => self.special_function_registers[MCS51_REGISTERS::P1 as usize],
            0x91 => self.special_function_registers[MCS51_REGISTERS::EXIF as usize],
            0x96 => self.special_function_registers[MCS51_REGISTERS::PSBANK as usize],
            0x98 => self.special_function_registers[MCS51_REGISTERS::SCON as usize],
            0x99 => self.special_function_registers[MCS51_REGISTERS::SBUF as usize],
            0xA0 => self.special_function_registers[MCS51_REGISTERS::SFR_EXEC_GO as usize],
            0xA1 => self.special_function_registers[MCS51_REGISTERS::SFR_EXEC_STATUS as usize],
            0xA2 => self.special_function_registers[MCS51_REGISTERS::SFR_REG_ADDRH as usize],
            0xA3 => self.special_function_registers[MCS51_REGISTERS::SFR_REG_ADDRL as usize],
            0xA4 => self.special_function_registers[MCS51_REGISTERS::SFR_REG_DATA_24 as usize],
            0xA5 => self.special_function_registers[MCS51_REGISTERS::SFR_REG_DATA_16 as usize],
            0xA6 => self.special_function_registers[MCS51_REGISTERS::SFR_REG_DATA_8 as usize],
            0xA7 => self.special_function_registers[MCS51_REGISTERS::SFR_REG_DATA_0 as usize],
            0xA8 => self.special_function_registers[MCS51_REGISTERS::IE as usize],
            0xB0 => self.special_function_registers[MCS51_REGISTERS::P3 as usize],
            0xB1 => self.special_function_registers[MCS51_REGISTERS::SFR_FLASH_CMD_R as usize],
            0xB2 => self.special_function_registers[MCS51_REGISTERS::SFR_FLASH_CMD as usize],
            0xB8 => self.special_function_registers[MCS51_REGISTERS::IP as usize],
            0xBC => self.special_function_registers[MCS51_REGISTERS::SFR_FLASH_CONFIG as usize],
            0xC2 => self.special_function_registers[MCS51_REGISTERS::SFR_SMI_REGH as usize],
            0xC3 => self.special_function_registers[MCS51_REGISTERS::SFR_SMI_REGL as usize],
            0xC4 => self.special_function_registers[MCS51_REGISTERS::SFR_SMI_DEV as usize],
            0xC5 => self.special_function_registers[MCS51_REGISTERS::SFR_SMI_PHYMASK as usize],

            0xC8 => self.special_function_registers[MCS51_REGISTERS::T2CON as usize],
            0xCA => self.special_function_registers[MCS51_REGISTERS::RCAP2L as usize],
            0xCB => self.special_function_registers[MCS51_REGISTERS::RCAP2H as usize],
            0xCC => self.special_function_registers[MCS51_REGISTERS::TL2 as usize],
            0xCD => self.special_function_registers[MCS51_REGISTERS::TH2 as usize],
            0xD0 => self.special_function_registers[MCS51_REGISTERS::PSW as usize],
            0xE0 => self.special_function_registers[MCS51_REGISTERS::ACC as usize],
            0xE8 => self.special_function_registers[MCS51_REGISTERS::EIE as usize],
            0xF0 => self.special_function_registers[MCS51_REGISTERS::B as usize],
            _ => 0,
        }
    }

    pub fn get_mut_addr(&mut self, address: u8) -> Option<&mut u8> {
        match address {
            0x00..=0x7F => self.ram.get_mut(address as usize),
            0x80 => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::SFR_FLASH_EXEC as usize),
            0x81 => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::SP as usize),
            0x82 => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::DPL1 as usize),
            0x83 => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::DPH1 as usize),
            0x84 => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::DPL2 as usize),
            0x85 => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::DPH2 as usize),
            0x86 => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::DPS as usize),
            0x87 => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::PCON as usize),
            0x88 => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::TCON as usize),
            0x89 => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::TMOD as usize),
            0x8A => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::TL0 as usize),
            0x8B => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::TL1 as usize),
            0x8C => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::TH0 as usize),
            0x8D => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::TH1 as usize),
            0x90 => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::P1 as usize),
            0x98 => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::SCON as usize),
            0x99 => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::SBUF as usize),
            0xA0 => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::SFR_EXEC_GO as usize),
            0xA8 => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::IE as usize),
            0xB0 => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::P3 as usize),
            0xB8 => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::IP as usize),
            0xC8 => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::T2CON as usize),
            0xCA => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::RCAP2L as usize),
            0xCB => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::RCAP2H as usize),
            0xCC => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::TL2 as usize),
            0xCD => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::TH2 as usize),
            0xD0 => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::PSW as usize),
            0xE0 => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::ACC as usize),
            0xF0 => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::B as usize),
            _ => None,
        }
    }

    pub fn read_idirect(&self, address: u8) -> Option<u8> {
        self.ram.get(address as usize).copied()
    }

    pub fn read(&self, address: u8) -> Option<&u8> {
        match address {
            0x00..=0x7F => self.ram.get(address as usize),
            0x80 => self
                .special_function_registers
                .get(MCS51_REGISTERS::SFR_FLASH_EXEC as usize),
            0x81 => self
                .special_function_registers
                .get(MCS51_REGISTERS::SP as usize),
            0x82 => self
                .special_function_registers
                .get(MCS51_REGISTERS::DPL1 as usize),
            0x83 => self
                .special_function_registers
                .get(MCS51_REGISTERS::DPH1 as usize),
            0x87 => self
                .special_function_registers
                .get(MCS51_REGISTERS::PCON as usize),
            0x88 => self
                .special_function_registers
                .get(MCS51_REGISTERS::TCON as usize),
            0x89 => self
                .special_function_registers
                .get(MCS51_REGISTERS::TMOD as usize),
            0x8A => self
                .special_function_registers
                .get(MCS51_REGISTERS::TL0 as usize),
            0x8B => self
                .special_function_registers
                .get(MCS51_REGISTERS::TL1 as usize),
            0x8C => self
                .special_function_registers
                .get(MCS51_REGISTERS::TH0 as usize),
            0x8D => self
                .special_function_registers
                .get(MCS51_REGISTERS::TH1 as usize),
            0x90 => self
                .special_function_registers
                .get(MCS51_REGISTERS::P1 as usize),
            0x98 => self
                .special_function_registers
                .get(MCS51_REGISTERS::SCON as usize),
            0x99 => self
                .special_function_registers
                .get(MCS51_REGISTERS::SBUF as usize),
            0xA0 => self
                .special_function_registers
                .get(MCS51_REGISTERS::SFR_EXEC_GO as usize),
            0xA8 => self
                .special_function_registers
                .get(MCS51_REGISTERS::IE as usize),
            0xB0 => self
                .special_function_registers
                .get(MCS51_REGISTERS::P3 as usize),
            0xB8 => self
                .special_function_registers
                .get(MCS51_REGISTERS::IP as usize),
            0xC8 => self
                .special_function_registers
                .get(MCS51_REGISTERS::T2CON as usize),
            0xCA => self
                .special_function_registers
                .get(MCS51_REGISTERS::RCAP2L as usize),
            0xCB => self
                .special_function_registers
                .get(MCS51_REGISTERS::RCAP2H as usize),
            0xCC => self
                .special_function_registers
                .get(MCS51_REGISTERS::TL2 as usize),
            0xCD => self
                .special_function_registers
                .get(MCS51_REGISTERS::TH2 as usize),
            0xD0 => self
                .special_function_registers
                .get(MCS51_REGISTERS::PSW as usize),
            0xE0 => self
                .special_function_registers
                .get(MCS51_REGISTERS::ACC as usize),
            0xF0 => self
                .special_function_registers
                .get(MCS51_REGISTERS::B as usize),
            _ => None,
        }
    }

    pub fn write(&mut self, address: u8, value: u8) {
        match address {
            0x00..=0x7F => self.ram[address as usize] = value,
            0x80 => {
                self.special_function_registers[MCS51_REGISTERS::SFR_FLASH_EXEC as usize] = value
            }
            0x81 => self.special_function_registers[MCS51_REGISTERS::SP as usize] = value,
            0x82 => self.special_function_registers[MCS51_REGISTERS::DPL1 as usize] = value,
            0x83 => self.special_function_registers[MCS51_REGISTERS::DPH1 as usize] = value,
            0x87 => self.special_function_registers[MCS51_REGISTERS::PCON as usize] = value,
            0x88 => self.special_function_registers[MCS51_REGISTERS::TCON as usize] = value,
            0x89 => self.special_function_registers[MCS51_REGISTERS::TMOD as usize] = value,
            0x8A => self.special_function_registers[MCS51_REGISTERS::TL0 as usize] = value,
            0x8B => self.special_function_registers[MCS51_REGISTERS::TL1 as usize] = value,
            0x8C => self.special_function_registers[MCS51_REGISTERS::TH0 as usize] = value,
            0x8D => self.special_function_registers[MCS51_REGISTERS::TH1 as usize] = value,
            0x90 => self.special_function_registers[MCS51_REGISTERS::P1 as usize] = value,
            0x98 => self.special_function_registers[MCS51_REGISTERS::SCON as usize] = value,
            0x99 => {
                println!("SBUF = {value}");
                if let Some((tx, _)) = &mut self.ext_uart_0 {
                    tx.try_send(value).is_ok();
                }
            }
            0xA0 => self.special_function_registers[MCS51_REGISTERS::SFR_EXEC_GO as usize] = value,
            0xA8 => self.special_function_registers[MCS51_REGISTERS::IE as usize] = value,
            0xB0 => self.special_function_registers[MCS51_REGISTERS::P3 as usize] = value,
            0xB8 => self.special_function_registers[MCS51_REGISTERS::IP as usize] = value,
            0xC8 => self.special_function_registers[MCS51_REGISTERS::T2CON as usize] = value,
            0xCA => self.special_function_registers[MCS51_REGISTERS::RCAP2L as usize] = value,
            0xCB => self.special_function_registers[MCS51_REGISTERS::RCAP2H as usize] = value,
            0xCC => self.special_function_registers[MCS51_REGISTERS::TL2 as usize] = value,
            0xCD => self.special_function_registers[MCS51_REGISTERS::TH2 as usize] = value,
            0xD0 => self.special_function_registers[MCS51_REGISTERS::PSW as usize] = value,
            0xE0 => self.special_function_registers[MCS51_REGISTERS::ACC as usize] = value,
            0xF0 => self.special_function_registers[MCS51_REGISTERS::B as usize] = value,
            _ => (),
        }
    }

    pub fn set_dptr(&mut self, value: u16) {
        self.write_sfr(MCS51_REGISTERS::DPH1, (value >> 8) as u8);
        self.write_sfr(MCS51_REGISTERS::DPL1, value as u8);
    }

    pub fn get_dptr(&mut self) -> u16 {
        let dph = self.read_sfr(MCS51_REGISTERS::DPH1);
        let dpl = self.read_sfr(MCS51_REGISTERS::DPL1);

        ((dph as u16) << 8) + dpl as u16
    }

    pub fn set_carry_flag(&mut self, value: bool) {
        let reg = self.read_sfr(MCS51_REGISTERS::PSW);
        if value {
            self.write_sfr(MCS51_REGISTERS::PSW, reg | 0x80);
        } else {
            self.write_sfr(MCS51_REGISTERS::PSW, reg & !0x80);
        }
    }

    pub fn get_carry_flag(&mut self) -> bool {
        self.read_sfr(MCS51_REGISTERS::PSW) & 0x80 != 0
    }

    pub fn set_aux_carry_flag(&mut self, value: bool) {
        let reg = self.read_sfr(MCS51_REGISTERS::PSW);
        if value {
            self.write_sfr(MCS51_REGISTERS::PSW, reg | 0x40);
        } else {
            self.write_sfr(MCS51_REGISTERS::PSW, reg & !0x40);
        }
    }

    pub fn get_aux_carry_flag(&mut self) -> bool {
        self.read_sfr(MCS51_REGISTERS::PSW) & 0x40 != 0
    }

    pub fn set_overflow_flag(&mut self, value: bool) {
        let reg = self.read_sfr(MCS51_REGISTERS::PSW);
        if value {
            self.write_sfr(MCS51_REGISTERS::PSW, reg | 0x04);
        } else {
            self.write_sfr(MCS51_REGISTERS::PSW, reg & !0x04);
        }
    }

    pub fn get_overflow_flag(&mut self) -> bool {
        self.read_sfr(MCS51_REGISTERS::PSW) & 0x04 != 0
    }

    pub fn get_accumulator(&self) -> u8 {
        self.special_function_registers[MCS51_REGISTERS::ACC as usize]
    }

    pub fn set_accumulator(&mut self, value: u8) {
        self.special_function_registers[MCS51_REGISTERS::ACC as usize] = value;
    }

    pub fn reset_registers(&mut self) {
        self.special_function_registers[MCS51_REGISTERS::SFR_FLASH_EXEC as usize] = 0xFF;
        self.special_function_registers[MCS51_REGISTERS::SP as usize] = 0x07;
        self.special_function_registers[MCS51_REGISTERS::DPL1 as usize] = 0x00;
        self.special_function_registers[MCS51_REGISTERS::DPH1 as usize] = 0x00;
        self.special_function_registers[MCS51_REGISTERS::PCON as usize] = 0x00;
        self.special_function_registers[MCS51_REGISTERS::TCON as usize] = 0x00;
        self.special_function_registers[MCS51_REGISTERS::TMOD as usize] = 0x00;
        self.special_function_registers[MCS51_REGISTERS::TL0 as usize] = 0x00;
        self.special_function_registers[MCS51_REGISTERS::TL1 as usize] = 0x00;
        self.special_function_registers[MCS51_REGISTERS::TH0 as usize] = 0x00;
        self.special_function_registers[MCS51_REGISTERS::TH1 as usize] = 0x00;
        self.special_function_registers[MCS51_REGISTERS::P1 as usize] = 0xFF;
        self.special_function_registers[MCS51_REGISTERS::SCON as usize] = 0x00;
        self.special_function_registers[MCS51_REGISTERS::SBUF as usize] = 0x00;
        self.special_function_registers[MCS51_REGISTERS::SFR_EXEC_GO as usize] = 0xFF;
        self.special_function_registers[MCS51_REGISTERS::IE as usize] = 0x00;
        self.special_function_registers[MCS51_REGISTERS::P3 as usize] = 0xFF;
        self.special_function_registers[MCS51_REGISTERS::IP as usize] = 0x00;
        self.special_function_registers[MCS51_REGISTERS::T2CON as usize] = 0x00;
        self.special_function_registers[MCS51_REGISTERS::RCAP2L as usize] = 0x00;
        self.special_function_registers[MCS51_REGISTERS::RCAP2H as usize] = 0x00;
        self.special_function_registers[MCS51_REGISTERS::TL2 as usize] = 0x00;
        self.special_function_registers[MCS51_REGISTERS::TH2 as usize] = 0x00;
        self.special_function_registers[MCS51_REGISTERS::PSW as usize] = 0x00;
        self.special_function_registers[MCS51_REGISTERS::ACC as usize] = 0x00;
        self.special_function_registers[MCS51_REGISTERS::B as usize] = 0x00;
    }

    pub fn next_instruction_debug_match(&mut self) {
        if self.pc as usize >= self.program.len() {
            self.pc = 0;
        }

        let _opcode = self.program[self.pc as usize];
        //self.opcode_dispatch_match(opcode);
    }

    pub fn next_instruction_debug_table(&mut self) {
        if self.pc as usize >= self.program.len() {
            self.pc = 0;
        }

        let opcode = self.program[self.pc as usize];
        self.opcode_dispatch_table(opcode);
    }

    pub fn set_u8(&mut self, addressing: MCS51_ADDRESSING, value: u8) {
        match addressing {
            MCS51_ADDRESSING::ACCUMULATOR => self.write_sfr(MCS51_REGISTERS::ACC, value),
            MCS51_ADDRESSING::REGISTER(reg) => self.write_register(reg, value),
            MCS51_ADDRESSING::DIRECT(offset) => {
                self.write(self.program[self.op_pc as usize + offset as usize], value)
            }
            MCS51_ADDRESSING::INDIRECT_Ri(reg) => self.write(self.read_register(reg), value),
            _ => {
                println!("Unsupported addressing mode");
            }
        }
    }

    pub fn get_u8_mut(&mut self, addressing: MCS51_ADDRESSING) -> Option<&mut u8> {
        match addressing {
            MCS51_ADDRESSING::ACCUMULATOR => self
                .special_function_registers
                .get_mut(MCS51_REGISTERS::ACC as usize),
            MCS51_ADDRESSING::REGISTER(reg) => self.get_register_mut(reg),
            MCS51_ADDRESSING::DIRECT(offset) => {
                self.get_mut_addr(self.program[self.op_pc as usize + offset as usize])
            }
            MCS51_ADDRESSING::INDIRECT_Ri(reg) => self.get_mut_addr(self.read_register(reg)),
            _ => {
                println!("Unsupported addressing mode");
                None
            }
        }
    }

    pub fn get_u8(&self, addressing: MCS51_ADDRESSING) -> Option<u8> {
        match addressing {
            MCS51_ADDRESSING::ACCUMULATOR => Some(self.read_sfr(MCS51_REGISTERS::ACC)),
            MCS51_ADDRESSING::REGISTER(reg) => Some(self.read_register(reg)),
            MCS51_ADDRESSING::DIRECT(offset) => Some(
                *self
                    .read(self.program[self.op_pc as usize + offset as usize])
                    .unwrap(),
            ),
            MCS51_ADDRESSING::INDIRECT_Ri(reg) => self.read(self.read_register(reg)).cloned(),
            MCS51_ADDRESSING::DATA(offset) => {
                Some(self.program[self.op_pc as usize + offset as usize])
            }
            _ => {
                println!("Unsupported addressing mode");
                None
            }
        }
    }

    pub fn get_i8(&self, addressing: MCS51_ADDRESSING) -> Option<i8> {
        match addressing {
            MCS51_ADDRESSING::DATA(offset) => {
                //Some(i8::from_be_bytes([*self.read(offset).unwrap()]))
                Some(i8::from_be_bytes([
                    self.program[self.op_pc as usize + offset as usize]
                ]))
            }
            _ => {
                println!("Unsupported addressing mode");
                None
            }
        }
    }

    pub fn get_u16(&self, addressing: MCS51_ADDRESSING) -> Option<u16> {
        match addressing {
            MCS51_ADDRESSING::ADDR_16 => {
                let offset: usize = self.op_pc as usize + 1;
                let mut data: [u8; 2] = [0; 2];
                data.copy_from_slice(&self.program[offset..offset + 2]);
                let addr = u16::from_be_bytes(data);

                Some(addr)
            }
            MCS51_ADDRESSING::DATA(off) => {
                let offset: usize = self.op_pc as usize + off as usize;

                let mut data: [u8; 2] = [0; 2];
                data.copy_from_slice(&self.program[offset..offset + 2]);
                let dat = u16::from_be_bytes(data);
                Some(dat)
            }
            _ => {
                println!("Unsupported addressing mode");
                None
            }
        }
    }

    pub fn get_u11(&self) -> u16 {
        let hi_byte = (self.program[self.op_pc as usize] as u16) << 3;
        let lo_byte = self.program[self.op_pc as usize + 1];
        let addr: u16 = hi_byte + lo_byte as u16;
        addr
    }

    /*
     */

    pub fn generate_opcode_array(&mut self) {
        self.dispatch[0x00] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_nop();
            cpu.opcode_additional_work("NOP", 0, 1);
        };
        self.dispatch[0x01] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_ajmp();
            cpu.opcode_additional_work("AJMP", 1, 0);
        };
        self.dispatch[0x02] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_ljmp(MCS51_ADDRESSING::ADDR_16);
            cpu.opcode_additional_work("LJMP", 1, 0);
        };
        self.dispatch[0x03] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_rr();
            cpu.opcode_additional_work("RR", 0, 1);
        };
        self.dispatch[0x04] = |cpu: &mut DW8051_RTL837x| {
            println!("Acc");
            cpu.op_inc(MCS51_ADDRESSING::ACCUMULATOR);
            cpu.opcode_additional_work("INC", 0, 1);
        };
        self.dispatch[0x05] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_inc(MCS51_ADDRESSING::DIRECT(1));
            cpu.opcode_additional_work("INC", 0, 2);
        };
        self.dispatch[0x06] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_inc(MCS51_ADDRESSING::INDIRECT_Ri(0));
            cpu.opcode_additional_work("INC", 0, 1);
        };
        self.dispatch[0x07] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_inc(MCS51_ADDRESSING::INDIRECT_Ri(1));
            cpu.opcode_additional_work("INC", 0, 1);
        };
        self.dispatch[0x08] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_inc(MCS51_ADDRESSING::REGISTER(0));
            cpu.opcode_additional_work("INC", 0, 1);
        };
        self.dispatch[0x09] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_inc(MCS51_ADDRESSING::REGISTER(1));
            cpu.opcode_additional_work("INC", 0, 1);
        };
        self.dispatch[0x0A] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_inc(MCS51_ADDRESSING::REGISTER(2));
            cpu.opcode_additional_work("INC", 0, 1);
        };
        self.dispatch[0x0B] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_inc(MCS51_ADDRESSING::REGISTER(3));
            cpu.opcode_additional_work("INC", 0, 1);
        };
        self.dispatch[0x0C] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_inc(MCS51_ADDRESSING::REGISTER(4));
            cpu.opcode_additional_work("INC", 0, 1);
        };
        self.dispatch[0x0D] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_inc(MCS51_ADDRESSING::REGISTER(5));
            cpu.opcode_additional_work("INC", 0, 1);
        };
        self.dispatch[0x0E] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_inc(MCS51_ADDRESSING::REGISTER(6));
            cpu.opcode_additional_work("INC", 0, 1);
        };
        self.dispatch[0x0F] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_inc(MCS51_ADDRESSING::REGISTER(7));
            cpu.opcode_additional_work("INC", 0, 1);
        };
        self.dispatch[0x10] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_jbc(MCS51_ADDRESSING::DATA(1), MCS51_ADDRESSING::DATA(2));
            cpu.opcode_additional_work("JBC", 1, 0);
        };
        self.dispatch[0x11] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_acall();
            cpu.opcode_additional_work("ACALL", 1, 0);
        };
        self.dispatch[0x12] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_lcall(MCS51_ADDRESSING::ADDR_16);
            cpu.opcode_additional_work("LCALL", 1, 0);
        };
        self.dispatch[0x13] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_rrc();
            cpu.opcode_additional_work("RRC", 0, 1);
        };
        self.dispatch[0x14] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_dec(MCS51_ADDRESSING::ACCUMULATOR);
            cpu.opcode_additional_work("DEC", 0, 1);
        };
        self.dispatch[0x15] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_dec(MCS51_ADDRESSING::DIRECT(1));
            cpu.opcode_additional_work("DEC", 0, 2);
        };
        self.dispatch[0x16] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_dec(MCS51_ADDRESSING::INDIRECT_Ri(0));
            cpu.opcode_additional_work("DEC", 0, 1);
        };
        self.dispatch[0x17] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_dec(MCS51_ADDRESSING::INDIRECT_Ri(1));
            cpu.opcode_additional_work("DEC", 0, 1);
        };
        self.dispatch[0x18] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_dec(MCS51_ADDRESSING::REGISTER(0));
            cpu.opcode_additional_work("DEC", 0, 1);
        };
        self.dispatch[0x19] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_dec(MCS51_ADDRESSING::REGISTER(1));
            cpu.opcode_additional_work("DEC", 0, 1);
        };
        self.dispatch[0x1A] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_dec(MCS51_ADDRESSING::REGISTER(2));
            cpu.opcode_additional_work("DEC", 0, 1);
        };
        self.dispatch[0x1B] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_dec(MCS51_ADDRESSING::REGISTER(3));
            cpu.opcode_additional_work("DEC", 0, 1);
        };
        self.dispatch[0x1C] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_dec(MCS51_ADDRESSING::REGISTER(4));
            cpu.opcode_additional_work("DEC", 0, 1);
        };
        self.dispatch[0x1D] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_dec(MCS51_ADDRESSING::REGISTER(5));
            cpu.opcode_additional_work("DEC", 0, 1);
        };
        self.dispatch[0x1E] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_dec(MCS51_ADDRESSING::REGISTER(6));
            cpu.opcode_additional_work("DEC", 0, 1);
        };
        self.dispatch[0x1F] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_dec(MCS51_ADDRESSING::REGISTER(7));
            cpu.opcode_additional_work("DEC", 0, 1);
        };
        self.dispatch[0x20] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_jb(MCS51_ADDRESSING::DATA(1), MCS51_ADDRESSING::DATA(2));
            cpu.opcode_additional_work("JB", 1, 0);
        };
        self.dispatch[0x21] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_ajmp();
            cpu.opcode_additional_work("AJMP", 1, 0);
        };
        self.dispatch[0x22] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_ret();
            cpu.opcode_additional_work("RET", 1, 0);
        };
        self.dispatch[0x23] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_rl();
            cpu.opcode_additional_work("RL", 0, 1);
        };
        self.dispatch[0x24] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_add(MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("ADD", 0, 2);
        };
        self.dispatch[0x25] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_add(MCS51_ADDRESSING::DIRECT(1));
            cpu.opcode_additional_work("ADD", 0, 2);
        };
        self.dispatch[0x26] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_add(MCS51_ADDRESSING::INDIRECT_Ri(0));
            cpu.opcode_additional_work("ADD", 0, 1);
        };
        self.dispatch[0x27] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_add(MCS51_ADDRESSING::INDIRECT_Ri(1));
            cpu.opcode_additional_work("ADD", 0, 1);
        };
        self.dispatch[0x28] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_add(MCS51_ADDRESSING::REGISTER(0));
            cpu.opcode_additional_work("ADD", 0, 1);
        };
        self.dispatch[0x29] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_add(MCS51_ADDRESSING::REGISTER(1));
            cpu.opcode_additional_work("ADD", 0, 1);
        };
        self.dispatch[0x2A] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_add(MCS51_ADDRESSING::REGISTER(2));
            cpu.opcode_additional_work("ADD", 0, 1);
        };
        self.dispatch[0x2B] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_add(MCS51_ADDRESSING::REGISTER(3));
            cpu.opcode_additional_work("ADD", 0, 1);
        };
        self.dispatch[0x2C] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_add(MCS51_ADDRESSING::REGISTER(4));
            cpu.opcode_additional_work("ADD", 0, 1);
        };
        self.dispatch[0x2D] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_add(MCS51_ADDRESSING::REGISTER(5));
            cpu.opcode_additional_work("ADD", 0, 1);
        };
        self.dispatch[0x2E] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_add(MCS51_ADDRESSING::REGISTER(6));
            cpu.opcode_additional_work("ADD", 0, 1);
        };
        self.dispatch[0x2F] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_add(MCS51_ADDRESSING::REGISTER(7));
            cpu.opcode_additional_work("ADD", 0, 1);
        };
        self.dispatch[0x30] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_jnb(MCS51_ADDRESSING::DATA(1), MCS51_ADDRESSING::DATA(2));
            cpu.opcode_additional_work("JNB", 1, 0);
        };
        self.dispatch[0x31] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_acall();
            cpu.opcode_additional_work("ACALL", 1, 0);
        };
        self.dispatch[0x32] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_reti();
            cpu.opcode_additional_work("RETI", 1, 0);
        };
        self.dispatch[0x33] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_rlc();
            cpu.opcode_additional_work("RLC", 1, 1);
        };
        self.dispatch[0x34] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_addc(MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("ADDC", 0, 2);
        };
        self.dispatch[0x35] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_addc(MCS51_ADDRESSING::DIRECT(1));
            cpu.opcode_additional_work("ADDC", 0, 2);
        };
        self.dispatch[0x36] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_addc(MCS51_ADDRESSING::INDIRECT_Ri(0));
            cpu.opcode_additional_work("ADDC", 0, 1);
        };
        self.dispatch[0x37] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_addc(MCS51_ADDRESSING::INDIRECT_Ri(1));
            cpu.opcode_additional_work("ADDC", 0, 1);
        };
        self.dispatch[0x38] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_addc(MCS51_ADDRESSING::REGISTER(0));
            cpu.opcode_additional_work("ADDC", 0, 1);
        };
        self.dispatch[0x39] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_addc(MCS51_ADDRESSING::REGISTER(1));
            cpu.opcode_additional_work("ADDC", 0, 1);
        };
        self.dispatch[0x3A] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_addc(MCS51_ADDRESSING::REGISTER(2));
            cpu.opcode_additional_work("ADDC", 0, 1);
        };
        self.dispatch[0x3B] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_addc(MCS51_ADDRESSING::REGISTER(3));
            cpu.opcode_additional_work("ADDC", 0, 1);
        };
        self.dispatch[0x3C] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_addc(MCS51_ADDRESSING::REGISTER(4));
            cpu.opcode_additional_work("ADDC", 0, 1);
        };
        self.dispatch[0x3D] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_addc(MCS51_ADDRESSING::REGISTER(5));
            cpu.opcode_additional_work("ADDC", 0, 1);
        };
        self.dispatch[0x3E] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_addc(MCS51_ADDRESSING::REGISTER(6));
            cpu.opcode_additional_work("ADDC", 0, 1);
        };
        self.dispatch[0x3F] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_addc(MCS51_ADDRESSING::REGISTER(7));
            cpu.opcode_additional_work("ADDC", 0, 1);
        };
        self.dispatch[0x40] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_jc(MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("JC", 1, 0);
        };
        self.dispatch[0x41] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_ajmp();
            cpu.opcode_additional_work("AJMP", 1, 0);
        };
        self.dispatch[0x42] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_orl(MCS51_ADDRESSING::DIRECT(1), MCS51_ADDRESSING::ACCUMULATOR);
            cpu.opcode_additional_work("ORL", 0, 2);
        };
        self.dispatch[0x43] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_orl(MCS51_ADDRESSING::DIRECT(1), MCS51_ADDRESSING::DATA(2));
            cpu.opcode_additional_work("ORL", 1, 3);
        };
        self.dispatch[0x44] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_orl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("ORL", 0, 2);
        };
        self.dispatch[0x45] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_orl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::DIRECT(1));
            cpu.opcode_additional_work("ORL", 0, 2);
        };
        self.dispatch[0x46] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_orl(
                MCS51_ADDRESSING::ACCUMULATOR,
                MCS51_ADDRESSING::INDIRECT_Ri(0),
            );
            cpu.opcode_additional_work("ORL", 0, 1);
        };
        self.dispatch[0x47] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_orl(
                MCS51_ADDRESSING::ACCUMULATOR,
                MCS51_ADDRESSING::INDIRECT_Ri(1),
            );
            cpu.opcode_additional_work("ORL", 0, 1);
        };
        self.dispatch[0x48] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_orl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(0));
            cpu.opcode_additional_work("ORL", 0, 1);
        };
        self.dispatch[0x49] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_orl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(1));
            cpu.opcode_additional_work("ORL", 0, 1);
        };
        self.dispatch[0x4A] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_orl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(2));
            cpu.opcode_additional_work("ORL", 0, 1);
        };
        self.dispatch[0x4B] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_orl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(3));
            cpu.opcode_additional_work("ORL", 0, 1);
        };
        self.dispatch[0x4C] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_orl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(4));
            cpu.opcode_additional_work("ORL", 0, 1);
        };
        self.dispatch[0x4D] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_orl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(5));
            cpu.opcode_additional_work("ORL", 0, 1);
        };
        self.dispatch[0x4E] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_orl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(6));
            cpu.opcode_additional_work("ORL", 0, 1);
        };
        self.dispatch[0x4F] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_orl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(7));
            cpu.opcode_additional_work("ORL", 0, 1);
        };
        self.dispatch[0x50] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_jnc(MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("JNC", 1, 0);
        };
        self.dispatch[0x51] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_acall();
            cpu.opcode_additional_work("ACALL", 1, 0);
        };
        self.dispatch[0x52] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_anl(MCS51_ADDRESSING::DIRECT(1), MCS51_ADDRESSING::ACCUMULATOR);
            cpu.opcode_additional_work("ANL", 0, 2);
        };
        self.dispatch[0x53] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_anl(MCS51_ADDRESSING::DIRECT(1), MCS51_ADDRESSING::DATA(2));
            cpu.opcode_additional_work("ANL", 1, 3);
        };
        self.dispatch[0x54] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_anl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("ANL", 0, 2);
        };
        self.dispatch[0x55] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_anl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::DIRECT(1));
            cpu.opcode_additional_work("ANL", 0, 2);
        };
        self.dispatch[0x56] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_anl(
                MCS51_ADDRESSING::ACCUMULATOR,
                MCS51_ADDRESSING::INDIRECT_Ri(0),
            );
            cpu.opcode_additional_work("ANL", 0, 1);
        };
        self.dispatch[0x57] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_anl(
                MCS51_ADDRESSING::ACCUMULATOR,
                MCS51_ADDRESSING::INDIRECT_Ri(1),
            );
            cpu.opcode_additional_work("ANL", 0, 1);
        };
        self.dispatch[0x58] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_anl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(0));
            cpu.opcode_additional_work("ANL", 0, 1);
        };
        self.dispatch[0x59] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_anl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(1));
            cpu.opcode_additional_work("ANL", 0, 1);
        };
        self.dispatch[0x5A] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_anl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(2));
            cpu.opcode_additional_work("ANL", 0, 1);
        };
        self.dispatch[0x5B] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_anl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(3));
            cpu.opcode_additional_work("ANL", 0, 1);
        };
        self.dispatch[0x5C] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_anl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(4));
            cpu.opcode_additional_work("ANL", 0, 1);
        };
        self.dispatch[0x5D] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_anl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(5));
            cpu.opcode_additional_work("ANL", 0, 1);
        };
        self.dispatch[0x5E] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_anl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(6));
            cpu.opcode_additional_work("ANL", 0, 1);
        };
        self.dispatch[0x5F] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_anl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(7));
            cpu.opcode_additional_work("ANL", 0, 1);
        };
        self.dispatch[0x60] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_jz(MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("JZ", 1, 0);
        };
        self.dispatch[0x61] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_ajmp();
            cpu.opcode_additional_work("AJMP", 1, 0);
        };
        self.dispatch[0x62] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_xrl(MCS51_ADDRESSING::DIRECT(1), MCS51_ADDRESSING::ACCUMULATOR);
            cpu.opcode_additional_work("XRL", 0, 2);
        };
        self.dispatch[0x63] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_orl(MCS51_ADDRESSING::DIRECT(1), MCS51_ADDRESSING::DATA(2));
            cpu.opcode_additional_work("XRL", 1, 3);
        };
        self.dispatch[0x64] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_xrl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("XRL", 0, 2);
        };
        self.dispatch[0x65] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_xrl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::DIRECT(1));
            cpu.opcode_additional_work("XRL", 0, 2);
        };
        self.dispatch[0x66] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_xrl(
                MCS51_ADDRESSING::ACCUMULATOR,
                MCS51_ADDRESSING::INDIRECT_Ri(0),
            );
            cpu.opcode_additional_work("XRL", 0, 1);
        };
        self.dispatch[0x67] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_xrl(
                MCS51_ADDRESSING::ACCUMULATOR,
                MCS51_ADDRESSING::INDIRECT_Ri(1),
            );
            cpu.opcode_additional_work("XRL", 0, 1);
        };
        self.dispatch[0x68] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_xrl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(0));
            cpu.opcode_additional_work("XRL", 0, 1);
        };
        self.dispatch[0x69] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_xrl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(1));
            cpu.opcode_additional_work("XRL", 0, 1);
        };
        self.dispatch[0x6A] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_xrl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(2));
            cpu.opcode_additional_work("XRL", 0, 1);
        };
        self.dispatch[0x6B] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_xrl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(3));
            cpu.opcode_additional_work("XRL", 0, 1);
        };
        self.dispatch[0x6C] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_xrl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(4));
            cpu.opcode_additional_work("XRL", 0, 1);
        };
        self.dispatch[0x6D] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_xrl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(5));
            cpu.opcode_additional_work("XRL", 0, 1);
        };
        self.dispatch[0x6E] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_xrl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(6));
            cpu.opcode_additional_work("XRL", 0, 1);
        };
        self.dispatch[0x6F] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_xrl(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(7));
            cpu.opcode_additional_work("XRL", 0, 1);
        };
        self.dispatch[0x70] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_jnz(MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("JNZ", 1, 0);
        };
        self.dispatch[0x71] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_acall();
            cpu.opcode_additional_work("ACALL", 1, 0);
        };
        self.dispatch[0x72] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_orl_c(MCS51_ADDRESSING::DATA(1), false);
            cpu.opcode_additional_work("ORL", 1, 2);
        };
        self.dispatch[0x73] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_jmp();
            cpu.opcode_additional_work("JMP", 1, 1);
        };
        self.dispatch[0x74] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("MOV", 0, 2);
        };
        self.dispatch[0x75] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::DIRECT(1), MCS51_ADDRESSING::DATA(2));
            cpu.opcode_additional_work("MOV", 0, 3);
        };
        self.dispatch[0x76] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::INDIRECT_Ri(0), MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("MOV", 0, 2);
        };
        self.dispatch[0x77] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::INDIRECT_Ri(1), MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("MOV", 0, 2);
        };
        self.dispatch[0x78] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::REGISTER(0), MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("MOV", 0, 2);
        };
        self.dispatch[0x79] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::REGISTER(1), MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("MOV", 0, 2);
        };
        self.dispatch[0x7A] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::REGISTER(2), MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("MOV", 0, 2);
        };
        self.dispatch[0x7B] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::REGISTER(3), MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("MOV", 0, 2);
        };
        self.dispatch[0x7C] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::REGISTER(4), MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("MOV", 0, 2);
        };
        self.dispatch[0x7D] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::REGISTER(5), MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("MOV", 0, 2);
        };
        self.dispatch[0x7E] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::REGISTER(6), MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("MOV", 0, 2);
        };
        self.dispatch[0x7F] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::REGISTER(7), MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("MOV", 0, 2);
        };
        self.dispatch[0x80] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_sjmp(MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("SJMP", 1, 0);
        };
        self.dispatch[0x81] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_ajmp();
            cpu.opcode_additional_work("AJMP", 1, 0);
        };
        self.dispatch[0x82] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_anl_c(MCS51_ADDRESSING::DATA(1), false);
            cpu.opcode_additional_work("ANL", 1, 2);
        };
        self.dispatch[0x83] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_movc_pc();
            cpu.opcode_additional_work("MOVC", 1, 1);
        };
        self.dispatch[0x84] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_div();
            cpu.opcode_additional_work("DIV", 3, 1);
        };
        self.dispatch[0x85] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::DIRECT(1), MCS51_ADDRESSING::DIRECT(2));
            cpu.opcode_additional_work("MOV", 1, 3);
        };
        self.dispatch[0x86] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(
                MCS51_ADDRESSING::DIRECT(1),
                MCS51_ADDRESSING::INDIRECT_Ri(0),
            );
            cpu.opcode_additional_work("MOV", 1, 2);
        };
        self.dispatch[0x87] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(
                MCS51_ADDRESSING::DIRECT(1),
                MCS51_ADDRESSING::INDIRECT_Ri(1),
            );
            cpu.opcode_additional_work("MOV", 1, 2);
        };
        self.dispatch[0x88] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::DIRECT(1), MCS51_ADDRESSING::REGISTER(0));
            cpu.opcode_additional_work("MOV", 1, 2);
        };
        self.dispatch[0x89] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::DIRECT(1), MCS51_ADDRESSING::REGISTER(1));
            cpu.opcode_additional_work("MOV", 1, 2);
        };
        self.dispatch[0x8A] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::DIRECT(1), MCS51_ADDRESSING::REGISTER(2));
            cpu.opcode_additional_work("MOV", 1, 2);
        };
        self.dispatch[0x8B] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::DIRECT(1), MCS51_ADDRESSING::REGISTER(3));
            cpu.opcode_additional_work("MOV", 1, 2);
        };
        self.dispatch[0x8C] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::DIRECT(1), MCS51_ADDRESSING::REGISTER(4));
            cpu.opcode_additional_work("MOV", 1, 2);
        };
        self.dispatch[0x8D] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::DIRECT(1), MCS51_ADDRESSING::REGISTER(5));
            cpu.opcode_additional_work("MOV", 1, 2);
        };
        self.dispatch[0x8E] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::DIRECT(1), MCS51_ADDRESSING::REGISTER(6));
            cpu.opcode_additional_work("MOV", 1, 2);
        };
        self.dispatch[0x8F] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::DIRECT(1), MCS51_ADDRESSING::REGISTER(7));
            cpu.opcode_additional_work("MOV", 1, 2);
        };
        self.dispatch[0x90] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov_dptr(MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("MOV", 1, 3);
        };
        self.dispatch[0x91] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_acall();
            cpu.opcode_additional_work("ACALL", 1, 1);
        };
        self.dispatch[0x92] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov_bit_c(MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("MOV", 1, 2);
        };
        self.dispatch[0x93] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_movc_dptr();
            cpu.opcode_additional_work("MOVC", 1, 1);
        };
        self.dispatch[0x94] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_subb(MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("SUBB", 0, 2);
        };
        self.dispatch[0x95] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_subb(MCS51_ADDRESSING::DIRECT(1));
            cpu.opcode_additional_work("SUBB", 0, 2);
        };
        self.dispatch[0x96] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_subb(MCS51_ADDRESSING::INDIRECT_Ri(0));
            cpu.opcode_additional_work("SUBB", 0, 1);
        };
        self.dispatch[0x97] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_subb(MCS51_ADDRESSING::INDIRECT_Ri(1));
            cpu.opcode_additional_work("SUBB", 0, 1);
        };
        self.dispatch[0x98] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_subb(MCS51_ADDRESSING::REGISTER(0));
            cpu.opcode_additional_work("SUBB", 0, 1);
        };
        self.dispatch[0x99] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_subb(MCS51_ADDRESSING::REGISTER(1));
            cpu.opcode_additional_work("SUBB", 0, 1);
        };
        self.dispatch[0x9A] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_subb(MCS51_ADDRESSING::REGISTER(2));
            cpu.opcode_additional_work("SUBB", 0, 1);
        };
        self.dispatch[0x9B] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_subb(MCS51_ADDRESSING::REGISTER(3));
            cpu.opcode_additional_work("SUBB", 0, 1);
        };
        self.dispatch[0x9C] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_subb(MCS51_ADDRESSING::REGISTER(4));
            cpu.opcode_additional_work("SUBB", 0, 1);
        };
        self.dispatch[0x9D] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_subb(MCS51_ADDRESSING::REGISTER(5));
            cpu.opcode_additional_work("SUBB", 0, 1);
        };
        self.dispatch[0x9E] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_subb(MCS51_ADDRESSING::REGISTER(6));
            cpu.opcode_additional_work("SUBB", 0, 1);
        };
        self.dispatch[0x9F] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_subb(MCS51_ADDRESSING::REGISTER(7));
            cpu.opcode_additional_work("SUBB", 0, 1);
        };
        self.dispatch[0xA0] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_orl_c(MCS51_ADDRESSING::DATA(1), true);
            cpu.opcode_additional_work("ORLC", 1, 2);
        };
        self.dispatch[0xA1] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_ajmp();
            cpu.opcode_additional_work("AJMP", 1, 1);
        };
        self.dispatch[0xA2] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov_c_bit(MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("MOV", 0, 2);
        };
        self.dispatch[0xA3] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_inc_dptr();
            cpu.opcode_additional_work("INC DPTR", 1, 1);
        };
        self.dispatch[0xA4] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mul();
            cpu.opcode_additional_work("MUL", 3, 1)
        };
        self.dispatch[0xA5] = |cpu: &mut DW8051_RTL837x| {
            cpu.opcode_additional_work("RESERVED", 0, 0);
        };
        self.dispatch[0xA6] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(
                MCS51_ADDRESSING::INDIRECT_Ri(0),
                MCS51_ADDRESSING::DIRECT(1),
            );
            cpu.opcode_additional_work("MOV", 1, 2)
        };
        self.dispatch[0xA7] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(
                MCS51_ADDRESSING::INDIRECT_Ri(1),
                MCS51_ADDRESSING::DIRECT(1),
            );
            cpu.opcode_additional_work("MOV", 1, 2)
        };
        self.dispatch[0xA8] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::REGISTER(0), MCS51_ADDRESSING::DIRECT(1));
            cpu.opcode_additional_work("MOV", 1, 2)
        };
        self.dispatch[0xA9] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::REGISTER(1), MCS51_ADDRESSING::DIRECT(1));
            cpu.opcode_additional_work("MOV", 1, 2)
        };
        self.dispatch[0xAA] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::REGISTER(2), MCS51_ADDRESSING::DIRECT(1));
            cpu.opcode_additional_work("MOV", 1, 2)
        };
        self.dispatch[0xAB] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::REGISTER(3), MCS51_ADDRESSING::DIRECT(1));
            cpu.opcode_additional_work("MOV", 1, 2)
        };
        self.dispatch[0xAC] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::REGISTER(4), MCS51_ADDRESSING::DIRECT(1));
            cpu.opcode_additional_work("MOV", 1, 2)
        };
        self.dispatch[0xAD] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::REGISTER(5), MCS51_ADDRESSING::DIRECT(1));
            cpu.opcode_additional_work("MOV", 1, 2)
        };
        self.dispatch[0xAE] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::REGISTER(6), MCS51_ADDRESSING::DIRECT(1));
            cpu.opcode_additional_work("MOV", 1, 2)
        };
        self.dispatch[0xAF] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::REGISTER(7), MCS51_ADDRESSING::DIRECT(1));
            cpu.opcode_additional_work("MOV", 1, 2)
        };
        self.dispatch[0xB0] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_anl_c(MCS51_ADDRESSING::DIRECT(1), true);
            cpu.opcode_additional_work("ANL", 1, 2)
        };
        self.dispatch[0xB1] = |_cpu: &mut DW8051_RTL837x| {};
        self.dispatch[0xB2] = |_cpu: &mut DW8051_RTL837x| {};
        self.dispatch[0xB3] = |_cpu: &mut DW8051_RTL837x| {};
        self.dispatch[0xB4] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_cjne(
                MCS51_ADDRESSING::ACCUMULATOR,
                MCS51_ADDRESSING::DATA(1),
                MCS51_ADDRESSING::DATA(2),
            );
            cpu.opcode_additional_work("CJNE", 2, 0)
        };
        self.dispatch[0xB5] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_cjne(
                MCS51_ADDRESSING::ACCUMULATOR,
                MCS51_ADDRESSING::DIRECT(1),
                MCS51_ADDRESSING::DATA(2),
            );
            cpu.opcode_additional_work("CJNE", 2, 0)
        };
        self.dispatch[0xB6] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_cjne(
                MCS51_ADDRESSING::INDIRECT_Ri(0),
                MCS51_ADDRESSING::DATA(1),
                MCS51_ADDRESSING::DATA(2),
            );
            cpu.opcode_additional_work("CJNE", 2, 0)
        };
        self.dispatch[0xB7] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_cjne(
                MCS51_ADDRESSING::INDIRECT_Ri(1),
                MCS51_ADDRESSING::DATA(1),
                MCS51_ADDRESSING::DATA(2),
            );
            cpu.opcode_additional_work("CJNE", 2, 0)
        };
        self.dispatch[0xB8] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_cjne(
                MCS51_ADDRESSING::REGISTER(0),
                MCS51_ADDRESSING::DATA(1),
                MCS51_ADDRESSING::DATA(2),
            );
            cpu.opcode_additional_work("CJNE", 2, 0)
        };
        self.dispatch[0xB9] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_cjne(
                MCS51_ADDRESSING::REGISTER(1),
                MCS51_ADDRESSING::DATA(1),
                MCS51_ADDRESSING::DATA(2),
            );
            cpu.opcode_additional_work("CJNE", 2, 0)
        };
        self.dispatch[0xBA] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_cjne(
                MCS51_ADDRESSING::REGISTER(2),
                MCS51_ADDRESSING::DATA(1),
                MCS51_ADDRESSING::DATA(2),
            );
            cpu.opcode_additional_work("CJNE", 2, 0)
        };
        self.dispatch[0xBB] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_cjne(
                MCS51_ADDRESSING::REGISTER(3),
                MCS51_ADDRESSING::DATA(1),
                MCS51_ADDRESSING::DATA(2),
            );
            cpu.opcode_additional_work("CJNE", 2, 0)
        };
        self.dispatch[0xBC] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_cjne(
                MCS51_ADDRESSING::REGISTER(4),
                MCS51_ADDRESSING::DATA(1),
                MCS51_ADDRESSING::DATA(2),
            );
            cpu.opcode_additional_work("CJNE", 2, 0)
        };
        self.dispatch[0xBD] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_cjne(
                MCS51_ADDRESSING::REGISTER(5),
                MCS51_ADDRESSING::DATA(1),
                MCS51_ADDRESSING::DATA(2),
            );
            cpu.opcode_additional_work("CJNE", 2, 0)
        };
        self.dispatch[0xBE] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_cjne(
                MCS51_ADDRESSING::REGISTER(6),
                MCS51_ADDRESSING::DATA(1),
                MCS51_ADDRESSING::DATA(2),
            );
            cpu.opcode_additional_work("CJNE", 2, 0)
        };
        self.dispatch[0xBF] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_cjne(
                MCS51_ADDRESSING::REGISTER(7),
                MCS51_ADDRESSING::DATA(1),
                MCS51_ADDRESSING::DATA(2),
            );
            cpu.opcode_additional_work("CJNE", 2, 0)
        };
        self.dispatch[0xC0] = |cpu: &mut DW8051_RTL837x| {
            let reg = cpu.get_u8(MCS51_ADDRESSING::DATA(1)).unwrap();
            cpu.op_push(MCS51_ADDRESSING::REGISTER(reg));
            cpu.opcode_additional_work("PUSH", 2, 2)
        };
        self.dispatch[0xC1] = |_cpu: &mut DW8051_RTL837x| {};
        self.dispatch[0xC2] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_clr(MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("CLR", 2, 2)
        };
        self.dispatch[0xC3] = |cpu: &mut DW8051_RTL837x| {
            cpu.set_carry_flag(false);
            cpu.opcode_additional_work("CLR C", 1, 1)
        };
        self.dispatch[0xC4] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_swap();
            cpu.opcode_additional_work("SWAP", 1, 1)
        };
        self.dispatch[0xC5] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_xch(MCS51_ADDRESSING::DIRECT(1));
            cpu.opcode_additional_work("XCH", 1, 2)
        };
        self.dispatch[0xC6] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_xch(MCS51_ADDRESSING::INDIRECT_Ri(0));
            cpu.opcode_additional_work("XCH", 1, 1)
        };
        self.dispatch[0xC7] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_xch(MCS51_ADDRESSING::INDIRECT_Ri(1));
            cpu.opcode_additional_work("XCH", 1, 1)
        };
        self.dispatch[0xC8] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_xch(MCS51_ADDRESSING::REGISTER(0));
            cpu.opcode_additional_work("XCH", 1, 1)
        };
        self.dispatch[0xC9] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_xch(MCS51_ADDRESSING::REGISTER(1));
            cpu.opcode_additional_work("XCH", 1, 1)
        };
        self.dispatch[0xCA] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_xch(MCS51_ADDRESSING::REGISTER(2));
            cpu.opcode_additional_work("XCH", 1, 1)
        };
        self.dispatch[0xCB] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_xch(MCS51_ADDRESSING::REGISTER(3));
            cpu.opcode_additional_work("XCH", 1, 1)
        };
        self.dispatch[0xCC] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_xch(MCS51_ADDRESSING::REGISTER(4));
            cpu.opcode_additional_work("XCH", 1, 1)
        };
        self.dispatch[0xCD] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_xch(MCS51_ADDRESSING::REGISTER(5));
            cpu.opcode_additional_work("XCH", 1, 1)
        };
        self.dispatch[0xCE] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_xch(MCS51_ADDRESSING::REGISTER(6));
            cpu.opcode_additional_work("XCH", 1, 1)
        };
        self.dispatch[0xCF] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_xch(MCS51_ADDRESSING::REGISTER(7));
            cpu.opcode_additional_work("XCH", 1, 1)
        };
        self.dispatch[0xD0] = |cpu: &mut DW8051_RTL837x| {
            let reg = cpu.get_u8(MCS51_ADDRESSING::DATA(1)).unwrap();
            cpu.op_pop(MCS51_ADDRESSING::REGISTER(reg));
            cpu.opcode_additional_work("POP", 2, 2)
        };
        self.dispatch[0xD1] = |_cpu: &mut DW8051_RTL837x| {};
        self.dispatch[0xD2] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_setb(MCS51_ADDRESSING::DATA(1));
            cpu.opcode_additional_work("SETB", 2, 2)
        };
        self.dispatch[0xD3] = |cpu: &mut DW8051_RTL837x| {
            cpu.set_carry_flag(true);
            cpu.opcode_additional_work("SETB", 1, 1)
        };
        self.dispatch[0xD4] = |_cpu: &mut DW8051_RTL837x| {};
        self.dispatch[0xD5] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_djnz(MCS51_ADDRESSING::DIRECT(1), MCS51_ADDRESSING::DATA(2), 3);
            cpu.opcode_additional_work("DJNZ", 2, 0)
        };
        self.dispatch[0xD6] = |_cpu: &mut DW8051_RTL837x| {};
        self.dispatch[0xD7] = |_cpu: &mut DW8051_RTL837x| {};
        self.dispatch[0xD8] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_djnz(MCS51_ADDRESSING::REGISTER(0), MCS51_ADDRESSING::DATA(1), 2);
            cpu.opcode_additional_work("DJNZ", 2, 0)
        };
        self.dispatch[0xD9] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_djnz(MCS51_ADDRESSING::REGISTER(1), MCS51_ADDRESSING::DATA(1), 2);
            cpu.opcode_additional_work("DJNZ", 2, 0)
        };
        self.dispatch[0xDA] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_djnz(MCS51_ADDRESSING::REGISTER(2), MCS51_ADDRESSING::DATA(1), 2);
            cpu.opcode_additional_work("DJNZ", 2, 0)
        };
        self.dispatch[0xDB] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_djnz(MCS51_ADDRESSING::REGISTER(3), MCS51_ADDRESSING::DATA(1), 2);
            cpu.opcode_additional_work("DJNZ", 2, 0)
        };
        self.dispatch[0xDC] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_djnz(MCS51_ADDRESSING::REGISTER(4), MCS51_ADDRESSING::DATA(1), 2);
            cpu.opcode_additional_work("DJNZ", 2, 0)
        };
        self.dispatch[0xDD] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_djnz(MCS51_ADDRESSING::REGISTER(5), MCS51_ADDRESSING::DATA(1), 2);
            cpu.opcode_additional_work("DJNZ", 2, 0)
        };
        self.dispatch[0xDE] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_djnz(MCS51_ADDRESSING::REGISTER(6), MCS51_ADDRESSING::DATA(1), 2);
            cpu.opcode_additional_work("DJNZ", 2, 0)
        };
        self.dispatch[0xDF] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_djnz(MCS51_ADDRESSING::REGISTER(7), MCS51_ADDRESSING::DATA(1), 2);
            cpu.opcode_additional_work("DJNZ", 2, 0)
        };
        self.dispatch[0xE0] = |cpu: &mut DW8051_RTL837x| {
            cpu.opcode_additional_work("MOVX A, @DPTR", 2, 1);
        };
        self.dispatch[0xE1] = |_cpu: &mut DW8051_RTL837x| {};
        self.dispatch[0xE2] = |cpu: &mut DW8051_RTL837x| {
            cpu.opcode_additional_work("MOVX A, @R0", 2, 1);
        };
        self.dispatch[0xE3] = |cpu: &mut DW8051_RTL837x| {
            cpu.opcode_additional_work("MOVX A, @R1", 2, 1);
        };
        self.dispatch[0xE4] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_clr(MCS51_ADDRESSING::ACCUMULATOR);
            cpu.opcode_additional_work("CLR A", 1, 1)
        };
        self.dispatch[0xE5] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::DIRECT(1));
            cpu.opcode_additional_work("MOV", 1, 2)
        };
        self.dispatch[0xE6] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(
                MCS51_ADDRESSING::ACCUMULATOR,
                MCS51_ADDRESSING::INDIRECT_Ri(0),
            );
            cpu.opcode_additional_work("MOV", 1, 1)
        };
        self.dispatch[0xE7] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(
                MCS51_ADDRESSING::ACCUMULATOR,
                MCS51_ADDRESSING::INDIRECT_Ri(1),
            );
            cpu.opcode_additional_work("MOV", 1, 1)
        };
        self.dispatch[0xE8] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(0));
            cpu.opcode_additional_work("MOV", 1, 1)
        };
        self.dispatch[0xE9] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(1));
            cpu.opcode_additional_work("MOV", 1, 1)
        };
        self.dispatch[0xEA] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(2));
            cpu.opcode_additional_work("MOV", 1, 1)
        };
        self.dispatch[0xEB] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(3));
            cpu.opcode_additional_work("MOV", 1, 1)
        };
        self.dispatch[0xEC] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(4));
            cpu.opcode_additional_work("MOV", 1, 1)
        };
        self.dispatch[0xED] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(5));
            cpu.opcode_additional_work("MOV", 1, 1)
        };
        self.dispatch[0xEE] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(6));
            cpu.opcode_additional_work("MOV", 1, 1)
        };
        self.dispatch[0xEF] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::ACCUMULATOR, MCS51_ADDRESSING::REGISTER(7));
            cpu.opcode_additional_work("MOV", 1, 1)
        };
        self.dispatch[0xF0] = |cpu: &mut DW8051_RTL837x| {
            cpu.opcode_additional_work("MOVX @DPTR, A", 2, 1);
        };
        self.dispatch[0xF1] = |_cpu: &mut DW8051_RTL837x| println!("DP: 0xF1");
        self.dispatch[0xF2] = |cpu: &mut DW8051_RTL837x| {
            cpu.opcode_additional_work("MOVX @R0, A", 2, 1);
        };
        self.dispatch[0xF3] = |cpu: &mut DW8051_RTL837x| {
            cpu.opcode_additional_work("MOVX @R1, A", 2, 1);
        };
        self.dispatch[0xF4] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_cpl_a();
            cpu.opcode_additional_work("CPL A", 1, 1);
        };
        self.dispatch[0xF5] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::DIRECT(1), MCS51_ADDRESSING::ACCUMULATOR);
            cpu.opcode_additional_work("MOV", 0, 2);
        };
        self.dispatch[0xF6] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(
                MCS51_ADDRESSING::INDIRECT_Ri(0),
                MCS51_ADDRESSING::ACCUMULATOR,
            );
            cpu.opcode_additional_work("MOV", 0, 1);
        };
        self.dispatch[0xF7] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(
                MCS51_ADDRESSING::INDIRECT_Ri(1),
                MCS51_ADDRESSING::ACCUMULATOR,
            );
            cpu.opcode_additional_work("MOV", 0, 1);
        };
        self.dispatch[0xF8] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::REGISTER(0), MCS51_ADDRESSING::ACCUMULATOR);
            cpu.opcode_additional_work("MOV", 0, 1);
        };
        self.dispatch[0xF9] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::REGISTER(1), MCS51_ADDRESSING::ACCUMULATOR);
            cpu.opcode_additional_work("MOV", 0, 1);
        };
        self.dispatch[0xFA] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::REGISTER(2), MCS51_ADDRESSING::ACCUMULATOR);
            cpu.opcode_additional_work("MOV", 0, 1);
        };
        self.dispatch[0xFB] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::REGISTER(3), MCS51_ADDRESSING::ACCUMULATOR);
            cpu.opcode_additional_work("MOV", 0, 1);
        };
        self.dispatch[0xFC] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::REGISTER(4), MCS51_ADDRESSING::ACCUMULATOR);
            cpu.opcode_additional_work("MOV", 0, 1);
        };
        self.dispatch[0xFD] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::REGISTER(5), MCS51_ADDRESSING::ACCUMULATOR);
            cpu.opcode_additional_work("MOV", 0, 1);
        };
        self.dispatch[0xFE] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::REGISTER(6), MCS51_ADDRESSING::ACCUMULATOR);
            cpu.opcode_additional_work("MOV", 0, 1);
        };
        self.dispatch[0xFF] = |cpu: &mut DW8051_RTL837x| {
            cpu.op_mov(MCS51_ADDRESSING::REGISTER(7), MCS51_ADDRESSING::ACCUMULATOR);
            cpu.opcode_additional_work("MOV", 0, 1);
        };
    }

    pub fn opcode_dispatch_table(&mut self, opcode: u8) {
        (self.dispatch[opcode as usize])(self);
    }

    pub fn opcode_additional_work(&mut self, _label: &str, cycles: u8, pc: u16) {
        if pc != 0 {
            self.pc += pc
        }
        if cycles != 0 {
            self.additional_cycles = cycles
        }
        if self.debug {
            println!("{:04x} : {}", self.pc, _label);
        }
    }

    /*
    Set Bit

    SETB sets the indicated bit to one. SETB can operate on·the carry flag or any directly
    addressable bit. No other flags are affected.
    */

    pub fn op_setb(&mut self, addr: MCS51_ADDRESSING) {
        let bit_addr = self.get_u8(addr).unwrap();
        self.write_bit(bit_addr, true);
    }

    /*
    Exchange Accumulator with byte variable

    XCH loads the Accumulator with the contents of the indicated variable, at the same time
    writing the original Accumulator contents to the indicated variable.
    */

    pub fn op_xch(&mut self, addr: MCS51_ADDRESSING) {
        let addr_val = self.get_u8(addr).unwrap();
        let acc_val = self.get_accumulator();

        self.set_accumulator(addr_val);
        self.set_u8(addr, acc_val);
    }

    /*
    Complement Accumulator

    Each bit of the Accumulator is logically complemented (one's complement).
    Bits which previously contained a one are changed to a zero and vice-versa.
    No flags are affected
    */

    pub fn op_cpl_a(&mut self) {
        let acc = self.get_accumulator();
        self.set_accumulator(!acc);
    }

    /*
    Swap nibbles within the Accumulator

    SWAP A interchanges the low- and high-order nibbles (four-bit fields) of the Accumulator
    (bits 3-0 and bits 7-4). The operation can also be thOUght of as a four-bit rotate instruction. No
    flags are affected.
    */

    pub fn op_swap(&mut self) {
        let acc = self.get_accumulator();
        self.set_accumulator(acc.rotate_right(4));
    }

    pub fn op_movx_a_ri(&mut self, reg: u8) {
        let src_addr = self.get_u8(MCS51_ADDRESSING::REGISTER(reg));
        if let Some(val) =
            src_addr.and_then(|src_addr| self.xdata.get(usize::from(src_addr)).copied())
        {
            self.set_accumulator(val);
        } else {
            println!("Error XDATA outside meme!")
        }
    }

    pub fn op_movx_ri_a(&mut self, reg: u8) {
        let _dest_addr = self.get_u8(MCS51_ADDRESSING::REGISTER(reg));
    }

    /*
    Decrement and Jump if Not Zero

    DJNZ decrements the location indicated by 1, and branches to the address indicated by the
    second operand if the resulting value is not zero. An original value of OOH will underflow to
    OFFH. No flags are affected. The branch destination would be computed by adding the signed
    relative-displacement value in the last instruction byte to the PC, after incrementing the PC to
    the fIrst byte of the following instruction.

    The location decremented may be a register or directly addressed byte.

    Note: When this instruction is used to modify an output port, the value used as the original
    port data will be read from the output data latch, not the input pins.
    */

    pub fn op_djnz(&mut self, addr: MCS51_ADDRESSING, rel: MCS51_ADDRESSING, pc_offset: u16) {
        let val = self.get_u8(addr).unwrap().wrapping_sub(1);
        self.set_u8(addr, val);
        self.pc += pc_offset;

        if val != 0 {
            let rel_val = self.get_i8(rel).unwrap();
            self.write_pc_reli(rel_val as i16);
        }
    }

    pub fn op_clr(&mut self, bit_addr: MCS51_ADDRESSING) {
        self.write_bit(self.get_u8(bit_addr).unwrap(), false);
    }

    pub fn op_cjne(
        &mut self,
        dest: MCS51_ADDRESSING,
        src: MCS51_ADDRESSING,
        rel: MCS51_ADDRESSING,
    ) {
        let dest_data = self.get_u8(dest).unwrap();
        let src_data = self.get_u8(src).unwrap();
        self.pc += 3;

        if dest_data != src_data {
            let code = self.get_i8(rel).unwrap();
            self.write_pc_reli(code as i16);
        }

        self.set_carry_flag(dest_data < src_data);
    }

    pub fn op_inc_dptr(&mut self) {
        let dptr = self.get_dptr();
        self.set_dptr(dptr.wrapping_add(1));
    }

    pub fn op_mul(&mut self) {
        let a = self.get_accumulator() as u16;
        let b = self.read_sfr(MCS51_REGISTERS::B) as u16;

        if a == 0 || b == 0 {
            self.write_sfr(MCS51_REGISTERS::B, 0);
            self.set_accumulator(0);
            self.set_overflow_flag(false);
        } else {
            let result = a * b;
            self.write_sfr(MCS51_REGISTERS::B, (result >> 8) as u8);
            self.set_accumulator((result & 0xFF) as u8);
            self.set_overflow_flag(result > 0xFF);
        }
        self.set_carry_flag(false);
    }

    /*
    SUBB subtracts the indicated variable and the carry flag together from the Accumulator,leaving the result in the Accumulator.

    SUBB sets the carry (borrow) flag if a borrow is needed for bit 7, and clears C otherwise.
    (If C was set before executing a SUBB instruction, this indicates that a borrow was needed for the previous step in a multiple precision subtraction, so
    the carry is subtracted from the Accumulator along with the source operand.) AC is set if aborrow is needed for bit 3,
    and cleared otherwise. OV is set if a borrow is needed into bit 6, but not into bit 7, or into bit 7, but not bit 6.When subtracting signed integers OV indicates a
    negative number produced when a negative value is subtracted from a positive value, or a positive result when a positive number is subtracted from a negative number.
    The source operand allows four addressing modes: register, direct, register-indirect, or immediate.

    Example

    The Accumulator holds 0C9H (11001001B), register 2 holds 54H (01010100B), and the carryflag is set.
    The instruction, SUBB  A,R2     will leave the value 74H (01110100B) in the accumulator, with the carry flag and AC clearedbut OV set.
    Notice that 0C9H minus 54H is 75H. The difference between this and the above result is due to the carry (borrow) flag being set before the operation.
    If the state of the carry is not knownbefore starting a single or multiple-precision subtraction,
    it should be explicitly cleared by a CLR C instruction.
    */

    pub fn op_subb(&mut self, src_addr: MCS51_ADDRESSING) {
        let src = self.get_u8(src_addr).unwrap();
        let acc = self.get_accumulator();

        let mut result = acc.wrapping_sub(src);

        if self.get_carry_flag() {
            result = result.wrapping_sub(1);
        }

        self.set_accumulator(result);
        // todo!();
    }

    pub fn op_mov_c_bit(&mut self, bit_addr: MCS51_ADDRESSING) {
        let bit = self.read_bit(self.get_u8(bit_addr).unwrap());
        self.set_carry_flag(bit);
    }

    pub fn op_mov_bit_c(&mut self, bit_addr: MCS51_ADDRESSING) {
        let cf = self.get_carry_flag();
        self.write_bit(self.get_u8(bit_addr).unwrap(), cf);
    }

    pub fn op_mov_dptr(&mut self, data16: MCS51_ADDRESSING) {
        let data = self.get_u16(data16).unwrap();
        self.set_dptr(data);
    }

    pub fn op_div(&mut self) {
        let b = self.read_sfr(MCS51_REGISTERS::B);
        self.set_carry_flag(false);

        if b == 0 {
            self.set_overflow_flag(true);
        } else {
            self.set_overflow_flag(false);
            let a = self.get_accumulator();
            let result = a / b;
            let remainder = a % b;

            self.set_accumulator(result);
            self.write_sfr(MCS51_REGISTERS::B, remainder);
        }
    }

    pub fn op_movc_pc(&mut self) {
        let pc = self.pc + 1;
        let acc = self.get_accumulator() as u16;
        let value = self.read_code_byte((pc + acc) as usize);
        self.set_accumulator(value);
    }

    pub fn op_movc_dptr(&mut self) {
        let acc = self.get_accumulator() as u16;
        let dptr = self.get_dptr();
        let value = self.read_code_byte((dptr + acc) as usize);
        self.set_accumulator(value);
    }

    pub fn op_jmp(&mut self) {
        let acc = self.get_accumulator() as u16;
        let dptr = self.get_dptr();
        self.pc = dptr.wrapping_add(acc);
    }

    pub fn op_jnz(&mut self, code_addr: MCS51_ADDRESSING) {
        let acc = self.get_accumulator();
        let code = self.get_i8(code_addr).unwrap();
        self.pc += 2;

        if acc != 0 {
            self.write_pc_reli(code as i16);
        }
    }

    pub fn op_jz(&mut self, code_addr: MCS51_ADDRESSING) {
        let acc = self.get_accumulator();
        let code = self.get_i8(code_addr).unwrap();
        self.pc += 2;

        if acc == 0 {
            self.write_pc_reli(code as i16);
        }
    }

    pub fn op_jnc(&mut self, code_addr: MCS51_ADDRESSING) {
        let cf = self.get_carry_flag();
        let code = self.get_i8(code_addr).unwrap();
        self.pc += 2;

        if !cf {
            self.write_pc_reli(code as i16);
        }
    }

    pub fn op_anl_c(&mut self, addr: MCS51_ADDRESSING, complement: bool) {
        let bit = self.read_bit(self.get_u8(addr).unwrap());
        let mut cf = self.get_carry_flag();

        if complement {
            cf &= !bit;
        } else {
            cf &= bit;
        }

        self.set_carry_flag(cf);
    }

    pub fn op_anl(&mut self, dest: MCS51_ADDRESSING, src: MCS51_ADDRESSING) {
        let op1 = self.get_u8(src).unwrap();
        let op2 = self.get_u8(dest).unwrap();

        let result = op1 & op2;

        self.set_u8(dest, result);
    }

    pub fn op_xrl(&mut self, dest: MCS51_ADDRESSING, src: MCS51_ADDRESSING) {
        let op1 = self.get_u8(src).unwrap();
        let op2 = self.get_u8(dest).unwrap();

        let result = op1 ^ op2;

        self.set_u8(dest, result);
    }

    pub fn op_orl_c(&mut self, addr: MCS51_ADDRESSING, complement: bool) {
        let bit = self.read_bit(self.get_u8(addr).unwrap());
        let mut cf = self.get_carry_flag();

        if complement {
            cf |= !bit;
        } else {
            cf |= bit;
        }

        self.set_carry_flag(cf);
    }

    pub fn op_orl(&mut self, dest: MCS51_ADDRESSING, src: MCS51_ADDRESSING) {
        let op1 = self.get_u8(src).unwrap();
        let op2 = self.get_u8(dest).unwrap();

        let result = op1 | op2;

        self.set_u8(dest, result);
    }

    pub fn op_jc(&mut self, code_addr: MCS51_ADDRESSING) {
        let cf = self.get_carry_flag();
        let code = self.get_i8(code_addr).unwrap();
        self.pc += 2;

        if cf {
            self.write_pc_reli(code as i16);
        }
    }

    pub fn op_reti(&mut self) {
        if self.interrupts.is_negative() {
            self.interrupts = self.interrupts.neg();
        } else {
            error!("Invalid RETI at PC {:04x}", self.pc);
        }
        let pc_hi = self.pop_stack() as u16;
        let pc_lo = self.pop_stack() as u16;
        self.pc = (pc_hi << 8) + pc_lo;
    }

    pub fn op_int(&mut self) {
        let int_nr = self.interrupts.trailing_zeros() as usize;
        if let Some(int_pc) = INT_VEC.get(int_nr).copied() {
            let pc = self.pc.to_be_bytes();
            self.push_stack(pc[0]);
            self.push_stack(pc[1]);
            self.interrupts = self.interrupts.neg();
            self.pc = int_pc;
        } else {
            error!("Invalid int_nr {int_nr}");
        }
    }

    pub fn op_mov(&mut self, dest: MCS51_ADDRESSING, src: MCS51_ADDRESSING) {
        let src_dat = self.get_u8(src).unwrap();
        self.set_u8(dest, src_dat);
    }

    pub fn op_ajmp(&mut self) {
        let offset = self.get_u11();
        self.pc += 2;
        self.pc &= 0xF800;
        self.pc += offset;
    }

    pub fn op_acall(&mut self) {
        let offset = self.get_u11();
        self.pc += 2;
        self.push_stack((self.pc & 0xFF) as u8);
        self.push_stack(((self.pc >> 8) & 0xFF) as u8);
        self.pc &= 0xF800;
        self.pc += offset;
    }

    pub fn op_add(&mut self, operand: MCS51_ADDRESSING) {
        let data = self.get_u8(operand).unwrap();
        let acc = self.get_accumulator();

        // Bit 3 overflow
        let tmp = (data as u16 & 0xF) + (acc as u16 & 0xF);
        self.set_aux_carry_flag(tmp > 0xF);

        // Bit 6 overflow
        let tmp = (data as u16 & 0x7F) + (acc as u16 & 0x7F);
        let ov = tmp > 0x7F;

        let result = data as u16 + acc as u16;
        let carry = result > 0xFF;

        self.set_overflow_flag(carry ^ ov);
        self.set_carry_flag(carry);

        self.set_accumulator((result & 0xFF) as u8);
    }

    pub fn op_addc(&mut self, operand: MCS51_ADDRESSING) {
        let data = self.get_u8(operand).unwrap();
        let acc = self.get_accumulator();
        let c = self.get_carry_flag() as u8;

        // Bit 3 overflow
        let tmp = (data as u16 & 0xF) + (acc as u16 & 0xF) + (c as u16);
        self.set_aux_carry_flag(tmp > 0xF);

        // Bit 6 overflow
        let tmp = (data as u16 & 0x7F) + (acc as u16 & 0x7F) + (c as u16);
        let ov = tmp > 0x7F;

        let result = data as u16 + acc as u16 + c as u16;
        let carry = result > 0xFF;

        self.set_overflow_flag(carry ^ ov);
        self.set_carry_flag(carry);

        self.set_accumulator((result & 0xFF) as u8);
    }

    pub fn op_lcall(&mut self, addr16: MCS51_ADDRESSING) {
        let new_pc = self.get_u16(addr16).unwrap();
        println!("LCALL: {new_pc:04x}");
        self.pc += 3;
        self.push_stack((self.pc & 0xFF) as u8);
        self.push_stack(((self.pc >> 8) & 0xFF) as u8);
        self.pc = new_pc;
    }

    pub fn op_jbc(&mut self, bit_addr: MCS51_ADDRESSING, code_addr: MCS51_ADDRESSING) {
        self.pc += 3;
        let bit_address = self.get_u8(bit_addr).unwrap();

        let bit: bool = self.read_bit(bit_address);

        if bit {
            self.write_bit(bit_address, false);
            let rel = self.get_i8(code_addr).unwrap();
            self.write_pc_reli(rel as i16);
        }
    }

    pub fn op_jnb(&mut self, bit_addr: MCS51_ADDRESSING, code_addr: MCS51_ADDRESSING) {
        self.pc += 3;
        let bit_address = self.get_u8(bit_addr).unwrap();

        let bit: bool = self.read_bit(bit_address);

        if !bit {
            let rel = self.get_i8(code_addr).unwrap();
            self.write_pc_reli(rel as i16);
        }
    }

    pub fn op_jb(&mut self, bit_addr: MCS51_ADDRESSING, code_addr: MCS51_ADDRESSING) {
        self.pc += 3;
        let bit_address = self.get_u8(bit_addr).unwrap();

        let bit: bool = self.read_bit(bit_address);

        if bit {
            let rel = self.get_i8(code_addr).unwrap();
            self.write_pc_reli(rel as i16);
        }
    }

    pub fn op_ret(&mut self) {
        let pc_hi = self.pop_stack() as u16;
        let pc_lo = self.pop_stack() as u16;
        self.pc = (pc_hi << 8) + pc_lo;
    }

    // Decrement
    pub fn op_dec(&mut self, operand: MCS51_ADDRESSING) {
        let op = self.get_u8_mut(operand).unwrap();
        *op = op.wrapping_sub(1);
    }

    // Increment
    pub fn op_inc(&mut self, operand: MCS51_ADDRESSING) {
        let op = self.get_u8_mut(operand).unwrap();
        *op = op.wrapping_add(1);
    }

    pub fn op_rr(&mut self) {
        let mut acc = self.get_accumulator();
        acc = acc.wrapping_shr(1);
        self.set_accumulator(acc);
    }

    pub fn op_rrc(&mut self) {
        let acc = self.get_accumulator();
        let underflow = acc & 1 != 0;
        let carry = self.get_carry_flag();
        self.set_accumulator(acc.overflowing_shr(1).0 + (carry as u8 * 0x80));
        self.set_carry_flag(underflow);
    }

    pub fn op_rl(&mut self) {
        let mut acc = self.get_accumulator();
        acc = acc.wrapping_shl(1);
        self.set_accumulator(acc);
    }

    pub fn op_rlc(&mut self) {
        let acc = self.get_accumulator();
        let overflow = acc & 0x80 != 0;
        let carry = self.get_carry_flag();
        self.set_accumulator(acc << (1 + carry as u8));
        self.set_carry_flag(overflow);
    }

    pub fn op_ljmp(&mut self, operand: MCS51_ADDRESSING) {
        let addr = self.get_u16(operand).unwrap();
        self.pc = addr;
    }

    pub fn op_sjmp(&mut self, addr: MCS51_ADDRESSING) {
        let addr_rel = self.get_i8(addr).unwrap();
        self.pc += 2;
        self.write_pc_reli(addr_rel as i16);
    }

    pub fn op_nop(&mut self) {}

    // Push
    pub fn op_push(&mut self, addr: MCS51_ADDRESSING) {
        let val = self.get_u8(addr).unwrap();
        self.push_stack(val);
    }

    // Pop
    pub fn op_pop(&mut self, addr: MCS51_ADDRESSING) {
        let val = self.pop_stack();
        let reg = self.get_u8_mut(addr).unwrap();
        *reg = val;
    }

    pub fn get_program_counter(&self) -> u16 {
        self.pc
    }
}

impl Default for DW8051_RTL837x {
    fn default() -> Self {
        Self::new()
    }
}

/*
trait IOComponent {
    fn get_pin(&self, pin: usize) -> bool;
    fn set_pin(&mut self, pin: usize, val: bool);

    fn get_port_u8(&self, port: usize) -> u8;
    fn set_port_u8(&mut self, port: usize, val: u8);

    fn get_port_u16(&self, port: usize) -> u16;
    fn set_port_u16(&mut self, port: usize, val: u16);
}*/

const INT_VEC: [u16; 13] = [
    0x33, 0x03, 0x0b, 0x13, 0x1b, 0x23, 0x2b, 0x3b, 0x43, 0x4b, 0x53, 0x5b, 0x63,
];

impl MCU<u8> for DW8051_RTL837x {
    fn clock(&mut self) {
        if self.additional_cycles > 0 {
            self.additional_cycles -= 1;
        } else {
            self.next_instruction();
        }
    }

    fn next_instruction(&mut self) {
        // jump interrupt
        if self.interrupts > 0 {
            self.op_int();
        }
        let Some(opcode) = self.program.get(self.pc as usize).copied() else {
            println!(
                "Error: Out of Program Mem: PC {:04x} MEM: {:04x}",
                self.pc,
                self.program.len()
            );
            return;
        };
        self.op_pc = self.pc;

        println!("\t PC {:04x} {:02x}", self.pc, opcode);

        self.run_opcode(opcode);
    }

    fn run_opcode(&mut self, opcode: u8) {
        // println!("Opcode {opcode:02x}");
        self.opcode_dispatch_table(opcode)
    }

    fn set_program(&mut self, program: Vec<u8>) {
        self.program = program;
    }

    fn setup(&mut self) {
        self.reset();
        self.reset_registers();
        self.generate_opcode_array();
    }

    fn reset(&mut self) {
        self.pc = 0;
        self.ram = [0; 256];
        self.additional_cycles = 0;
        self.reset_registers();
    }

    fn run(&mut self) {
        let program_len = self.program.len() as u16;
        while self.pc < program_len {
            self.next_instruction();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc::{Receiver, Sender, channel};

    #[test]
    fn serial_interrupt() {
        let mut mcu = DW8051_RTL837x::new();
        mcu.setup();

        mcu.debug = true;

        let (send_tx, mut recv_tx) = channel(32);
        let (mut send_rx, recv_rx) = channel(32);

        mcu.ext_uart_0 = Some((send_tx, recv_rx));

        let prg = include_bytes!("../../../data/rtlinstall.bin");

        mcu.set_program(prg.to_vec());

        mcu.next_instruction();

        assert_eq!(mcu.get_program_counter(), 0x16);
        assert_eq!(mcu.get_stack_pointer(), 0x7);

        mcu.next_instruction();

        assert_eq!(mcu.get_stack_pointer(), 0x7);

        mcu.next_instruction();

        assert_eq!(mcu.get_program_counter(), 0x1d3);

        let mut char: Option<u8> = None;

        for _ in 1..50 {
            mcu.next_instruction();
            if let Ok(val) = recv_tx.try_recv() {
                char = Some(val);
                break;
            }
        }
        assert_eq!(char, Some(b'I'));
    }
}
