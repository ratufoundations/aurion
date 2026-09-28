#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opcode {
    Push(u64),
    Pop,
    Dup,
    Swap,
    Jmp(usize),
    JmpIf(usize),
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Lt,
    Gt,
    GetCaller,
    GetCallValue,
    LoadStorage,
    StoreStorage,
    Return,
    Revert(u64),
}

impl Opcode {
    pub fn gas_cost(&self) -> u64 {
        match self {
            Self::Pop | Self::Dup | Self::Swap => 1,
            Self::Push(_)
            | Self::Add
            | Self::Sub
            | Self::Eq
            | Self::Lt
            | Self::Gt
            | Self::GetCallValue => 2,
            Self::Mul | Self::Jmp(_) => 3,
            Self::JmpIf(_) => 4,
            Self::Div | Self::GetCaller => 5,
            Self::LoadStorage => 20,
            Self::StoreStorage => 100,
            Self::Return | Self::Revert(_) => 0,
        }
    }
}
