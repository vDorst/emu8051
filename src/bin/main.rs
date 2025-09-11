#![allow(dead_code)]
use microchip_rs::decompiler::mcs51::*;
use microchip_rs::mcus::mcs51::*;
use microchip_rs::mcus::pic16f628a::*;
use microchip_rs::traits::component::*;
use rustyline::Editor;
use rustyline::error::ReadlineError;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::time::Instant;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn register_operations_16f628a() {
        let mut mcu = PIC16F628A::new();
        mcu.reset();

        mcu.set_bank(0);
        assert_eq!(
            *mcu.get_register(PIC16F628A_REGISTERS::STATUS).unwrap(),
            0b00011000
        );
        mcu.set_bank(1);
        assert_eq!(
            *mcu.get_register(PIC16F628A_REGISTERS::STATUS).unwrap(),
            0b00111000
        );
        mcu.set_bank(0);

        let porta = mcu.get_register_mut(PIC16F628A_REGISTERS::PORTA).unwrap();
        *porta = 0x0F;
        let porta2 = mcu.get_memory_address_mut(0x05).unwrap();
        assert_eq!(*porta2, 0x0F);
        assert_eq!(
            *mcu.get_register(PIC16F628A_REGISTERS::PORTA).unwrap(),
            0x0F
        );
    }

    #[test]
    fn register_operations_mcs51() {
        let mut mcu = MCS51::new();
        mcu.debug = true;
        mcu.setup();

        assert_eq!(mcu.get_accumulator(), 0);
        mcu.set_program(vec![
            0x04, // Increment Accumulator
            0x04, // Increment Accumulator
            0x04, // Increment Accumulator
            0x09, // Increment Register 1
            0x09, // Increment Register 1
            0x14, // Decrement Accumulator
            0x14, // Decrement Accumulator
            0x19, // Decrement Register 1
            0x09, // Increment Register 1
        ]);
        println!("hier");
        mcu.next_instruction();
        assert_eq!(mcu.get_accumulator(), 1);
        mcu.next_instruction();
        mcu.next_instruction();
        assert_eq!(mcu.get_accumulator(), 3);
        mcu.next_instruction();
        mcu.next_instruction();
        assert_eq!(mcu.read_register(1), 2);
        mcu.next_instruction();
        mcu.next_instruction();
        assert_eq!(mcu.get_accumulator(), 1);
        mcu.next_instruction();
        assert_eq!(mcu.read_register(1), 1);

        //Switch bank
        mcu.write(0xD0, 0b00001000);
        mcu.next_instruction();
        assert_eq!(mcu.read_register(1), 1);
    }

    #[test]
    fn banking_operations_mcs51() {
        let mut mcu = MCS51::new();
        mcu.reset();
        mcu.write(0xD0, 0b00000000);
        assert_eq!(mcu.get_current_register_bank(), 0);
        mcu.write(0xD0, 0b00001000);
        assert_eq!(mcu.get_current_register_bank(), 1);
        mcu.write(0xD0, 0b00010000);
        assert_eq!(mcu.get_current_register_bank(), 2);
        mcu.write(0xD0, 0b00011000);
        assert_eq!(mcu.get_current_register_bank(), 3);
    }

    #[test]
    fn bit_operations_mcs51() {
        let mut mcu = MCS51::new();
        mcu.reset();

        assert_eq!(mcu.read_bit(0x60), false);
        assert_eq!(mcu.read_bit(0x61), false);
        assert_eq!(mcu.read_bit(0x62), false);

        mcu.write_bit(0x60, true);
        assert_eq!(mcu.get_accumulator(), 0);
        assert_eq!(*mcu.read(0x2c).unwrap(), 0);

        mcu.write_bit(0x61, true);
        assert_eq!(mcu.get_accumulator(), 0);
        assert_eq!(*mcu.read(0x2c).unwrap(), 0);
        assert_eq!(mcu.read_bit(0x61), true);

        mcu.write_bit(0x62, true);
        assert_eq!(mcu.get_accumulator(), 0);
        assert_eq!(*mcu.read(0x2c).unwrap(), 0);
        assert_eq!(mcu.read_bit(0x62), true);

        mcu.write_bit(0xE0, true);
        assert_eq!(mcu.get_accumulator(), 1);
        mcu.write_bit(0xE7, true);
        assert_eq!(mcu.get_accumulator(), 129);
    }

    #[test]
    fn bit_mov_operations_mcs51() {
        let mut mcu = MCS51::new();
        mcu.setup();

        mcu.set_program(vec![
            0x04, // Increment Accumulator
            0x04, // Increment Accumulator
            0x04, // Increment Accumulator
            0x09, // Increment Register 1
            0x09, // Increment Register 1
            0x09, // Increment Register 1
            0x74, 0xFE, // Store 0xFE in accumulator
            0x79, 0xFD, // Store 0xFD in R1
        ]);
        for _i in 0..8 {
            mcu.next_instruction();
        }

        mcu.next_instruction();
        assert_eq!(mcu.get_accumulator(), 0xFE);

        mcu.next_instruction();
        assert_eq!(mcu.read_register(1), 0xFD);
    }

    #[test]
    fn add_operations_mcs51() {
        let mut mcu = MCS51::new();
        mcu.setup();

        mcu.set_program(vec![
            0x74, 0xC3, // Store 0xC3 in Accumulator
            0x79, 0xAA, // Store 0xAA in R1
            0x29, //Add R1 to accumulator
        ]);
        mcu.next_instruction();
        mcu.next_instruction();
        mcu.next_instruction();
        assert_eq!(mcu.read_register(1), 0xAA);
        assert_eq!(mcu.get_accumulator(), 0x6D);
        assert_eq!(mcu.get_aux_carry_flag(), false);
        assert_eq!(mcu.get_carry_flag(), true);
        assert_eq!(mcu.get_overflow_flag(), true);
    }
}

fn test_emulator_16f628a() {
    let mut mcu = PIC16F628A::new();

    /*
    mcu.set_program(vec![
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0b10_1000_00000000 // GOTO 0
    ]);
    */
    //0b100000000

    /*
    mcu.set_program(vec![
        0b00_1010_1_1110000, // Increment address 70h and store back in f
        0b00_1010_1_1110000, // Increment address 70h and store back in f
        0b00_1010_1_1110000, // Increment address 70h and store back in f
        0b00_1010_1_1110000, // Increment address 70h and store back in f
        0b00_1010_1_1110000, // Increment address 70h and store back in f
        0b00_1010_1_1110000, // Increment address 70h and store back in f
        0b00_1010_1_1110000, // Increment address 70h and store back in f
        0b00_1010_1_1110000, // Increment address 70h and store back in f
        0b00_1010_1_1110000, // Increment address 70h and store back in f
        0b00_1010_1_1110000, // Increment address 70h and store back in f
        0b10_1000_00000000 // GOTO 0
    ]);
    */

    /*
    mcu.set_program(vec![
        0b00_0001_0_0000000, // Clear W
        0b00_0001_0_0000000, // Clear W
        0b00_0001_0_0000000, // Clear W
        0b00_0001_0_0000000, // Clear W
        0b00_0001_0_0000000, // Clear W
        0b00_0001_0_0000000, // Clear W
        0b00_0001_0_0000000, // Clear W
        0b00_0001_0_0000000, // Clear W
        0b00_0001_0_0000000, // Clear W
        0b00_0001_0_0000000, // Clear W
        0b10_1000_00000000 // GOTO 0
    ]);
    */

    mcu.set_program(vec![
        0b00_0001_1_1110000, // Clear f at 70h
        0b00_0001_1_1110000, // Clear f at 70h
        0b00_0001_1_1110000, // Clear f at 70h
        0b00_0001_1_1110000, // Clear f at 70h
        0b00_0001_1_1110000, // Clear f at 70h
        0b00_0001_1_1110000, // Clear f at 70h
        0b00_0001_1_1110000, // Clear f at 70h
        0b00_0001_1_1110000, // Clear f at 70h
        0b00_0001_1_1110000, // Clear f at 70h
        0b00_0001_1_1110000, // Clear f at 70h
        0b10_1000_00000000,  // GOTO 0
    ]);

    for _j in 0..10 {
        mcu.reset();

        let iterations = 1000000000;
        let now = Instant::now();
        for _i in 0..iterations {
            mcu.next_instruction();
        }

        let time_us = now.elapsed().as_micros();
        let time_ns = now.elapsed().as_nanos();
        let time_us_inst = time_us as f64 / iterations as f64;
        let time_ns_inst = time_ns as f64 / iterations as f64;
        println!(
            "({:.3}ns/inst) ({:.3} GHz) ({:.3} MHz)",
            time_ns_inst,
            1.0 / time_ns_inst,
            1.0 / time_us_inst
        );
    }
}

fn test_emulator_mcs51() {
    let mut mcu = MCS51::new();

    test_emulator(
        &mut mcu,
        vec![
            0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x02, 0x00,
            0x00, // Jump to beginning
        ],
    );

    /*
    test_emulator(&mut mcu, vec![
        0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, // Jump to beginning
    ]);
    */
}

fn test_emulator(mcu: &mut dyn MCU<u8>, program: Vec<u8>) {
    mcu.setup();
    mcu.set_program(program);

    for _j in 0..10 {
        mcu.reset();

        let iterations = 1000000000;
        let now = Instant::now();
        for _i in 0..iterations {
            mcu.next_instruction();
        }

        let time_us = now.elapsed().as_micros();
        let time_ns = now.elapsed().as_nanos();
        let time_us_inst = time_us as f64 / iterations as f64;
        let time_ns_inst = time_ns as f64 / iterations as f64;
        println!(
            "({:.3}ns/inst) ({:.3} GHz) ({:.3} MHz)",
            time_ns_inst,
            1.0 / time_ns_inst,
            1.0 / time_us_inst
        );
    }
}

fn run_mcs51(program: Vec<u8>) {
    let mut mcu = MCS51::new();
    mcu.setup();
    mcu.set_program(program);
    mcu.run();
    println!("{}", mcu.read_register(2));
}

fn get_file_as_byte_vec(filename: &str) -> Vec<u8> {
    let mut f = File::open(filename).expect("no file found");
    let metadata = std::fs::metadata(filename).expect("unable to read metadata");
    let mut buffer = vec![0; metadata.len() as usize];
    f.read(&mut buffer).expect("buffer overflow");
    buffer
}

fn test_decompile_mcs51_2(program: Vec<u8>, out_file: &str) {
    let mut dec = MCS51_Decompiler::new();
    dec.program = program;

    /*
    let mut next: u16 = 0;
    let mut code = String::new();
    for _i in 0..512 {
        let v = dec.get_instruction(next);
        code.push_str(format!("{}", v).as_str());
        code.push('\n');
        next = v.next[0];
    }

    fs::write("data/code.asm", code).expect("Unable to write file");
    */

    dec.decompile(0);
    dec.write_to_file(out_file);
}

fn test_decompile_mcs51() {
    test_decompile_mcs51_2(
        get_file_as_byte_vec(r#"../data/V2-10_raw.bin"#),
        "data/code_2_10.asm",
    )
}

fn repl_mcs51(filename: &str) {
    let mut f = File::open(filename).expect("no file found");
    let metadata = fs::metadata(filename).expect("unable to read metadata");
    let mut buffer = vec![0; metadata.len() as usize];
    f.read(&mut buffer).expect("buffer overflow");

    let mut mcu = MCS51::new();
    mcu.generate_opcode_array();
    mcu.set_program(buffer.clone());
    //mcu.debug = true;

    let mut decomp = MCS51_Decompiler::new();
    decomp.program = buffer;
    decomp.decompile(0);

    let mut prev_pc: u16 = 0;

    let breakpoints: Vec<u16> = vec![0x5DD6];

    let mut rl = rustyline::DefaultEditor::new().unwrap();

    loop {
        let readline = rl.readline(">> ");

        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str());

                match line.as_str() {
                    "exit" => {
                        break;
                    }

                    "" => {}

                    _ => {
                        if line.starts_with("wait ") {
                            let pattern = line.replace("wait ", "");

                            loop {
                                let pc = &mcu.pc;
                                let inst = decomp.get_instruction(*pc);
                                mcu.next_instruction();
                                println!("{}", inst);

                                if inst.to_string().contains(&pattern) {
                                    println!("{}", inst);
                                    break;
                                }
                            }
                        } else {
                            let pc = &mcu.pc;
                            let inst = decomp.get_instruction(*pc);
                            println!("{}", inst);

                            mcu.next_instruction();
                        }
                    }
                }
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
}

fn run_mcs51_file(filename: &str) {
    let mut f = File::open(filename).expect("no file found");
    let metadata = fs::metadata(filename).expect("unable to read metadata");
    let mut buffer = vec![0; metadata.len() as usize];
    f.read(&mut buffer).expect("buffer overflow");

    let mut mcu = MCS51::new();
    mcu.generate_opcode_array();
    mcu.set_program(buffer.clone());
    //mcu.debug = true;

    let mut decomp = MCS51_Decompiler::new();
    decomp.program = buffer;
    decomp.decompile(0);

    let mut prev_pc: u16 = 0;

    let breakpoints: Vec<u16> = vec![0x5DD6];

    loop {
        let pc = &mcu.pc;
        //println!("{:0x} {:0x}", pc, decomp.program[*pc as usize]);
        //let inst = decomp.instructions[pc].clone();
        let inst = decomp.get_instruction(*pc);
        println!("{}", inst);

        /*
        if *pc != 0 && prev_pc == *pc {
            break;
        } else {
            prev_pc = *pc;
            mcu.next_instruction();
        }
        */
        mcu.next_instruction();
    }
}

fn main() {
    test_emulator_mcs51();
    //repl_mcs51("data/1594462804_raw.bin");
    //test_decompile_mcs51();

    //run_mcs51(vec![0x78, 0x39, 0x79, 0x05, 0x7B, 0x10, 0xE8, 0x13, 0x50, 0x01, 0x0A, 0xBB, 0x08, 0x01, 0xE9, 0xDB, 0xF6]);
    //test_decompile_mcs51_2(vec![0x78, 0x39, 0x79, 0x05, 0x7B, 0x10, 0xE8, 0x13, 0x50, 0x01, 0x0A, 0xBB, 0x08, 0x01, 0xE9, 0xDB, 0xF6], "R:\\out.asm")
}
