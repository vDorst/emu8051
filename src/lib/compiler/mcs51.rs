pub enum MCS51_Compiler_Argument {
    Label(String),
    Data(u8),
    Data16(u16),
    Register(u8),
    Sfr(String),
}

pub struct MCS51_Compiler_Instruction {
    pub mnemonic: String,
    pub arguments: Vec<MCS51_Compiler_Argument>,
}

impl MCS51_Compiler_Instruction {
    pub fn from_string() -> MCS51_Compiler_Instruction {
        MCS51_Compiler_Instruction {
            mnemonic: "".to_owned(),
            arguments: vec![],
        }
    }
}

pub struct MCS51_Compiler {}
