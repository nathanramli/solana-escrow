use solana_program::program_error::ProgramError;
use thiserror::Error;

#[derive(Error, Debug, Clone, Copy)]
pub enum EscrowError {
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