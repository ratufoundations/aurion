use aurion_criptografi::PublicKeyBytes;

#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub caller: PublicKeyBytes,
    pub contract_address: PublicKeyBytes,
    pub call_value: u64,
    pub block_height: u64,
}

impl ExecutionContext {
    #[must_use]
    pub fn new(
        caller: PublicKeyBytes,
        contract_address: PublicKeyBytes,
        call_value: u64,
        block_height: u64,
    ) -> Self {
        Self {
            caller,
            contract_address,
            call_value,
            block_height,
        }
    }
}
