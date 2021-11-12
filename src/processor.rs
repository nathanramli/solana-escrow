use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack},
    pubkey::Pubkey,
    sysvar::{rent::Rent, Sysvar},
};

use spl_token::state::Account as TokenAccount;

use crate::{error::EscrowError, instruction::EscrowInstruction, state::Escrow};

pub struct Processor;
impl Processor {
    pub fn processor(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        let instruction = EscrowInstruction::unpack(instruction_data)?;

        match instruction {
            EscrowInstruction::InitEscrow { amount } => {
                msg!("Instruction: InitEscrow");
                Self::processor_init_escrow(accounts, amount, program_id)
            }
            EscrowInstruction::Exchange { amount } => {
                msg!("Instruction: Exchange");
                Self::processor_exchange(accounts, amount, program_id)
            }
        }
    }

    pub fn processor_init_escrow(
        accounts: &[AccountInfo],
        amount: u64,
        program_id: &Pubkey,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let initializer = next_account_info(account_info_iter)?;

        if !initializer.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        // This token account should be writeable, but no need to check it
        // because the transaction will fail when the program try to write it
        let temp_token_account = next_account_info(account_info_iter)?;
        let temp_token_account_state =
            TokenAccount::unpack(&temp_token_account.try_borrow_data()?)?;

        // You might ask yourself, "why do we check that the token_to_receive_account is actually owned by the token program but don't do the same for the temp_token_account?".
        // The answer is that later on in the function we will ask the token program to transfer ownership of the temp_token_account to the PDA.
        // This transfer will fail if the temp_token_account is not owned by the token program, because - as I'm sure you remember - only programs that own accounts may change accounts.
        // Hence, there is no need for us to add another check here.
        let token_to_receive_account = next_account_info(account_info_iter)?;
        if *token_to_receive_account.owner != spl_token::id() {
            return Err(ProgramError::IncorrectProgramId);
        }

        let escrow_account = next_account_info(account_info_iter)?;
        let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;

        if !rent.is_exempt(escrow_account.lamports(), escrow_account.data_len()) {
            return Err(EscrowError::NotRentExempt.into());
        }

        let mut escrow_info = Escrow::unpack_unchecked(&escrow_account.try_borrow_data()?)?;
        if escrow_info.is_initialized() {
            return Err(ProgramError::AccountAlreadyInitialized);
        }

        escrow_info.is_initialized = true;
        escrow_info.initializer_pubkey = *initializer.key;
        escrow_info.temp_token_account_pubkey = *temp_token_account.key;
        escrow_info.initializer_token_to_receive_account_pubkey = *token_to_receive_account.key;
        escrow_info.expected_amount = amount;

        Escrow::pack(escrow_info, &mut escrow_account.try_borrow_mut_data()?)?;

        let (pda, _bump_seed) = Pubkey::find_program_address(&[b"escrow"], program_id);

        let token_program = next_account_info(account_info_iter)?;
        let owner_change_ix = spl_token::instruction::set_authority(
            token_program.key,
            temp_token_account.key,
            Some(&pda),
            spl_token::instruction::AuthorityType::AccountOwner,
            initializer.key,
            &[&initializer.key],
        )?;

        msg!("Calling the token program to transfer token account ownership...");
        invoke(
            &owner_change_ix,
            &[
                temp_token_account.clone(),
                initializer.clone(),
                token_program.clone(),
            ],
        )?;

        Ok(())
    }

    pub fn processor_exchange(
        accounts: &[AccountInfo],
        amount: u64,
        program_id: &Pubkey,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let taker_account = next_account_info(account_info_iter)?;

        if !taker_account.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        let taker_token_to_send_account = next_account_info(account_info_iter)?;

        let taker_token_to_receive_account = next_account_info(account_info_iter)?;

        let pda_token_account = next_account_info(account_info_iter)?;
        let pda_token_account_state = TokenAccount::unpack(&pda_token_account.try_borrow_data()?)?;
        let (pda, bump_seed) = Pubkey::find_program_address(&[b"escrow"], program_id);

        if amount != pda_token_account_state.amount {
            return Err(EscrowError::ExpectedAmountMismatch.into());
        }

        let initializer_account = next_account_info(account_info_iter)?;

        let initializer_token_to_receive_account = next_account_info(account_info_iter)?;

        let escrow_account = next_account_info(account_info_iter)?;
        let escrow_info = Escrow::unpack(&escrow_account.try_borrow_data()?)?;

        if *pda_token_account.key != escrow_info.temp_token_account_pubkey {
            return Err(ProgramError::InvalidAccountData);
        }

        if escrow_info.initializer_pubkey != *initializer_account.key {
            return Err(ProgramError::InvalidAccountData);
        }

        if escrow_info.initializer_token_to_receive_account_pubkey
            != *initializer_token_to_receive_account.key
        {
            return Err(ProgramError::InvalidAccountData);
        }

        let token_program = next_account_info(account_info_iter)?;

        let ix_transfer_to_initializer = spl_token::instruction::transfer(
            token_program.key,
            taker_token_to_send_account.key,
            initializer_token_to_receive_account.key,
            taker_account.key,
            &[taker_account.key],
            amount,
        )?;
        invoke(
            &ix_transfer_to_initializer,
            &[
                token_program.clone(),
                taker_token_to_send_account.clone(),
                initializer_token_to_receive_account.clone(),
                taker_account.clone(),
            ],
        )?;

        let pda_account = next_account_info(account_info_iter)?;

        let ix_transfer_to_taker = spl_token::instruction::transfer(
            token_program.key,
            pda_token_account.key,
            taker_token_to_receive_account.key,
            &pda,
            &[&pda],
            pda_token_account_state.amount,
        )?;
        invoke_signed(
            &ix_transfer_to_taker,
            &[
                token_program.clone(),
                pda_token_account.clone(),
                taker_token_to_receive_account.clone(),
                pda_account.clone(),
            ],
            &[&[&b"escrow"[..], &[bump_seed]]],
        )?;

        let ix_close_pda_account = spl_token::instruction::close_account(
            token_program.key,
            pda_token_account.key,
            initializer_account.key,
            &pda,
            &[&pda],
        )?;
        invoke_signed(
            &ix_close_pda_account,
            &[
                token_program.clone(),
                pda_token_account.clone(),
                initializer_account.clone(),
                pda_account.clone(),
            ],
            &[&[&b"escrow"[..], &[bump_seed]]],
        )?;

        // gave back the lamports that was used for renting the escrow account space
        **initializer_account.lamports.borrow_mut() = initializer_account
            .lamports()
            .checked_add(escrow_account.lamports())
            .ok_or(EscrowError::AmountOverflow)?;
        // always clear the value inside the account after closing the account
        **escrow_account.lamports.borrow_mut() = 0;
        *escrow_account.try_borrow_mut_data()? = &mut [];

        Ok(())
    }
}
