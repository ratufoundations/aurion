use crate::{
    context::ExecutionContext, error::VmError, gas::GasMeter, opcodes::Opcode,
    storage::ContractStorage,
};

pub const MAX_STACK_DEPTH: usize = 1024;

pub struct AurionVm<'a> {
    context: &'a ExecutionContext,
    storage: &'a mut ContractStorage,
    stack: Vec<u64>,
    gas_meter: GasMeter,
}

impl<'a> AurionVm<'a> {
    pub fn new(
        context: &'a ExecutionContext,
        storage: &'a mut ContractStorage,
        gas_limit: u64,
    ) -> Self {
        Self {
            context,
            storage,
            stack: Vec::with_capacity(32),
            gas_meter: GasMeter::new(gas_limit),
        }
    }
    pub fn gas_consumed(&self) -> u64 {
        self.gas_meter.consumed
    }
    pub fn execute(&mut self, code: &[Opcode]) -> Result<Option<u64>, VmError> {
        let mut shadow_storage = self.storage.clone();
        let mut pc = 0;
        while pc < code.len() {
            let op = &code[pc];
            self.gas_meter.consume(op.gas_cost())?;
            match op {
                Opcode::Push(val) => {
                    if self.stack.len() >= MAX_STACK_DEPTH {
                        return Err(VmError::StackOverflow(MAX_STACK_DEPTH));
                    }
                    self.stack.push(*val);
                }
                Opcode::Pop => {
                    self.pop()?;
                }
                Opcode::Dup => {
                    let top = *self.stack.last().ok_or(VmError::StackUnderflow)?;
                    if self.stack.len() >= MAX_STACK_DEPTH {
                        return Err(VmError::StackOverflow(MAX_STACK_DEPTH));
                    }
                    self.stack.push(top);
                }
                Opcode::Swap => {
                    let len = self.stack.len();
                    if len < 2 {
                        return Err(VmError::StackUnderflow);
                    }
                    self.stack.swap(len - 1, len - 2);
                }
                Opcode::Add => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.stack
                        .push(a.checked_add(b).ok_or(VmError::ArithmeticOverflow)?);
                }
                Opcode::Sub => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.stack
                        .push(a.checked_sub(b).ok_or(VmError::ArithmeticOverflow)?);
                }
                Opcode::Mul => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.stack
                        .push(a.checked_mul(b).ok_or(VmError::ArithmeticOverflow)?);
                }
                Opcode::Div => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    if b == 0 {
                        return Err(VmError::DivisionByZero);
                    }
                    self.stack.push(a / b);
                }
                Opcode::Eq => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.stack.push(if a == b { 1 } else { 0 });
                }
                Opcode::Lt => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.stack.push(if a < b { 1 } else { 0 });
                }
                Opcode::Gt => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.stack.push(if a > b { 1 } else { 0 });
                }
                Opcode::Jmp(target) => {
                    if *target >= code.len() {
                        return Err(VmError::InstructionPointerOutOfBounds(*target));
                    }
                    pc = *target;
                    continue;
                }
                Opcode::JmpIf(target) => {
                    let cond = self.pop()?;
                    if cond != 0 {
                        if *target >= code.len() {
                            return Err(VmError::InstructionPointerOutOfBounds(*target));
                        }
                        pc = *target;
                        continue;
                    }
                }
                Opcode::GetCaller => {
                    let caller_id =
                        u64::from_le_bytes(self.context.caller[0..8].try_into().unwrap());
                    self.stack.push(caller_id);
                }
                Opcode::GetCallValue => {
                    self.stack.push(self.context.call_value);
                }
                Opcode::LoadStorage => {
                    let key = self.pop()?;
                    self.stack
                        .push(shadow_storage.get(&self.context.contract_address, key));
                }
                Opcode::StoreStorage => {
                    let val = self.pop()?;
                    let key = self.pop()?;
                    shadow_storage.set(&self.context.contract_address, key, val);
                }
                Opcode::Return => {
                    *self.storage = shadow_storage;
                    return Ok(self.stack.last().copied());
                }
                Opcode::Revert(code) => {
                    return Err(VmError::ExplicitRevert(*code));
                }
            }
            pc += 1;
        }
        *self.storage = shadow_storage;
        Ok(self.stack.last().copied())
    }
    #[inline(always)]
    fn pop(&mut self) -> Result<u64, VmError> {
        self.stack.pop().ok_or(VmError::StackUnderflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurion_criptografi::Keypair;
    #[test]
    fn test_counter_smart_contract_execution() {
        let caller_kp = Keypair::generate();
        let contract_addr = Keypair::generate().public_key_bytes();
        let ctx = ExecutionContext::new(caller_kp.public_key_bytes(), contract_addr, 0, 1);
        let mut storage = ContractStorage::new();
        let counter_bytecode = vec![
            Opcode::Push(100),
            Opcode::LoadStorage,
            Opcode::Push(1),
            Opcode::Add,
            Opcode::Dup,
            Opcode::Push(100),
            Opcode::Swap,
            Opcode::StoreStorage,
            Opcode::Return,
        ];
        {
            let mut vm = AurionVm::new(&ctx, &mut storage, 10_000);
            assert_eq!(vm.execute(&counter_bytecode).unwrap(), Some(1));
            assert_eq!(storage.get(&contract_addr, 100), 1);
        }
        {
            let mut vm = AurionVm::new(&ctx, &mut storage, 10_000);
            assert_eq!(vm.execute(&counter_bytecode).unwrap(), Some(2));
            assert_eq!(storage.get(&contract_addr, 100), 2);
        }
    }
    #[test]
    fn test_revert_rollbacks_storage_changes() {
        let caller_kp = Keypair::generate();
        let contract_addr = Keypair::generate().public_key_bytes();
        let ctx = ExecutionContext::new(caller_kp.public_key_bytes(), contract_addr, 0, 1);
        let mut storage = ContractStorage::new();
        let failing_bytecode = vec![
            Opcode::Push(999),
            Opcode::Push(50),
            Opcode::StoreStorage,
            Opcode::Revert(403),
        ];
        let mut vm = AurionVm::new(&ctx, &mut storage, 10_000);
        let err = vm.execute(&failing_bytecode).unwrap_err();
        assert_eq!(err, VmError::ExplicitRevert(403));
        assert_eq!(storage.get(&contract_addr, 50), 0);
    }
    #[test]
    fn test_infinite_loop_caught_by_gas_meter() {
        let caller_kp = Keypair::generate();
        let contract_addr = Keypair::generate().public_key_bytes();
        let ctx = ExecutionContext::new(caller_kp.public_key_bytes(), contract_addr, 0, 1);
        let mut storage = ContractStorage::new();
        let infinite_loop = vec![Opcode::Jmp(0)];
        let mut vm = AurionVm::new(&ctx, &mut storage, 50);
        assert_eq!(vm.execute(&infinite_loop).unwrap_err(), VmError::OutOfGas);
    }
}
