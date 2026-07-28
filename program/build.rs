use std::env;
use std::fmt::Write;
use std::fs;
use std::path::PathBuf;

use rustdom_x::superscalar::{generate_codegen_programs, CodegenOpcode};

mod epoch {
    include!("src/epoch.rs");
}

const PROGRAM_COUNT: usize = 8;

fn main() {
    println!("cargo:rerun-if-changed=src/epoch.rs");

    let programs = generate_codegen_programs(&epoch::RANDOMX_SEED, PROGRAM_COUNT);
    assert_eq!(programs.len(), PROGRAM_COUNT);

    let mut source = String::from(
        "use rustdom_x::common::{mulh, smulh};\n\
         use rustdom_x::memory::SeedMemory;\n\n\
         #[inline(always)]\n\
         pub fn init_dataset_item(seed_memory: &SeedMemory, item_num: u64) -> [u64; 8] {\n\
             let mut registers = [0u64; 8];\n\
             let mut register_value = item_num;\n\
             registers[0] = item_num.wrapping_add(1).wrapping_mul(6364136223846793005);\n\
             registers[1] = registers[0] ^ 9298411001130361340;\n\
             registers[2] = registers[0] ^ 12065312585734608966;\n\
             registers[3] = registers[0] ^ 9306329213124626780;\n\
             registers[4] = registers[0] ^ 5281919268842080866;\n\
             registers[5] = registers[0] ^ 10536153434571861004;\n\
             registers[6] = registers[0] ^ 3398623926847679864;\n\
             registers[7] = registers[0] ^ 9549104520008361294;\n",
    );

    for (program_index, program) in programs.iter().enumerate() {
        writeln!(source, "    // Superscalar program {program_index}.").unwrap();
        for instruction in &program.instructions {
            let dst = instruction.dst;
            let src = instruction.src;
            let immediate = instruction.immediate;
            match instruction.opcode {
                CodegenOpcode::IsubR => writeln!(
                    source,
                    "    registers[{dst}] = registers[{dst}].wrapping_sub(registers[{src}]);"
                ),
                CodegenOpcode::IxorR => writeln!(
                    source,
                    "    registers[{dst}] ^= registers[{src}];"
                ),
                CodegenOpcode::IaddRs => writeln!(
                    source,
                    "    registers[{dst}] = registers[{dst}].wrapping_add(registers[{src}] << {immediate});"
                ),
                CodegenOpcode::ImulR => writeln!(
                    source,
                    "    registers[{dst}] = registers[{dst}].wrapping_mul(registers[{src}]);"
                ),
                CodegenOpcode::IrorC => writeln!(
                    source,
                    "    registers[{dst}] = registers[{dst}].rotate_right({immediate}u32);"
                ),
                CodegenOpcode::IaddC => writeln!(
                    source,
                    "    registers[{dst}] = registers[{dst}].wrapping_add(0x{immediate:016x});"
                ),
                CodegenOpcode::IxorC => writeln!(
                    source,
                    "    registers[{dst}] ^= 0x{immediate:016x};"
                ),
                CodegenOpcode::ImulhR => writeln!(
                    source,
                    "    registers[{dst}] = mulh(registers[{dst}], registers[{src}]);"
                ),
                CodegenOpcode::IsmulhR => writeln!(
                    source,
                    "    registers[{dst}] = smulh(registers[{dst}], registers[{src}]);"
                ),
                CodegenOpcode::ImulRcp => writeln!(
                    source,
                    "    registers[{dst}] = registers[{dst}].wrapping_mul(0x{immediate:016x});"
                ),
            }
            .unwrap();
        }
        writeln!(
            source,
            "    seed_memory.xor_cache_line(register_value, &mut registers);"
        )
        .unwrap();
        if program_index + 1 != programs.len() {
            writeln!(
                source,
                "    register_value = registers[{}];",
                program.address_register
            )
            .unwrap();
        }
    }
    source.push_str("    registers\n}\n");

    let output = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set"))
        .join("static_superscalar.rs");
    fs::write(output, source).expect("writing fixed-epoch superscalar source");
}
