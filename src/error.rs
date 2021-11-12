use solana_program::program_error::ProgramError;
use thiserror::Error;

#[derive(Error, Debug, Clone, Copy)]
pub enum EscrowError {
    #[error("Amount Overflow")]
    AmountOverflow,

    #[error("Expected Amount Mismatch")]
    ExpectedAmountMismatch,

    #[error("Invalid Amount")]
    InvalidAmount,
    
    #[error("Invalid Instruction")]
    InvalidInstruction,
    
    #[error("Not Rent Exempt")]
    NotRentExempt,
}

impl From<EscrowError> for ProgramError {
    fn from(e: EscrowError) -> Self {
        ProgramError::Custom(e as u32)
    }
}