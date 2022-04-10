use thiserror::Error;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    program_pack::{Pack, Sealed},
    pubkey::Pubkey,
};
use std::convert::{TryInto};

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use std::mem;

#[derive(Error, Debug, Copy, Clone)]
pub enum GreetingError {
    /// Invalid instruction
    #[error("Invalid Instruction")]
    InvalidInstruction,
    /// Not Rent Exempt
    #[error("Not Rent Exempt")]
    NotRentExempt,
}

impl From<GreetingError> for ProgramError {
    fn from(e: GreetingError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

/// Define the type of state stored in accounts
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub struct GreetingAccount {
    /// number of greetings
    pub counter: u32,
    pub free_counter: u64,
}

impl Sealed for GreetingAccount { }

impl Pack for GreetingAccount {
    const LEN: usize = 12;
    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let src = array_ref![src, 0, GreetingAccount::LEN];
        let (
            counter,
            free_counter
        ) = array_refs![src, 4, 8];
        Ok(GreetingAccount {
            counter: u32::from_le_bytes(*counter),
            free_counter: u64::from_le_bytes(*free_counter)
        })
    }

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let dst = array_mut_ref![dst, 0, GreetingAccount::LEN];
        let (
            counter_dst,
            free_counter_dst
        ) = mut_array_refs![dst, 4, 8];

        let GreetingAccount {
            counter,
            free_counter,
        } = self;
        *counter_dst = counter.to_le_bytes();
        *free_counter_dst = free_counter.to_le_bytes();
    }
}
pub enum GreetingInstruction {
    /// Accounts expected;
    ///
    /// 0. `[signer]`   The account of the person initializing the escrow
    /// 1. `[writable]` Temporary token account that should be created prior to this instruction and owned by the initializer
    /// 2. `[]`         The initializer's token account for the token they will receive should the trade go throught
    /// 3. `[writable]` The escrow account, it will hold all necessary info about the trade
    /// 4. `[]`         The rent sysvar
    /// 5. `[]`         The token program

    InitGreeting {
        // The amount party A expects to receive of token Y
        amount: u64,
    },
}

impl GreetingInstruction {
    /// Unpacks a byte buffer into a [GreetingInstruction](enum.GreetingInstruction.html)
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (tag, rest) = input.split_first().ok_or(GreetingError::InvalidInstruction)?;

        Ok(match tag {
            0 => Self::InitGreeting {
                amount: Self::unpack_amount(rest)?,
            },
            _ => return Err(GreetingError::InvalidInstruction.into()),
        })
    }

    fn unpack_amount(input: &[u8]) -> Result<u64, ProgramError> {
        let amount = input
            .get(..8)
            .and_then(|slice| slice.try_into().ok())
            .map(u64::from_le_bytes)
            .ok_or(GreetingError::InvalidInstruction)?;
        Ok(amount)
    }
}

// Declare and export the program's entrypoint
entrypoint!(process_entrypoint);

fn process_entrypoint(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    Processor::process(program_id, accounts, instruction_data)
}

pub struct Processor;

impl Processor {
    pub fn process(
        program_id: &Pubkey, // Public key of the account the hello world program was loaded into
        accounts: &[AccountInfo], // The account to say hello to
        instruction_data: &[u8], // Ignored, all helloworld instructions are hellos
    ) -> ProgramResult {
        msg!("Hello World Rust program entrypoint");
        let instruction = GreetingInstruction::unpack(instruction_data)?;
        match instruction {
            GreetingInstruction::InitGreeting { amount } => {
                msg!("Instruction: InitGreeting");
                Self::process_greeting(program_id,accounts, amount, instruction_data)
            }
        }
    }
    // Program entrypoint's implementation
    pub fn process_greeting(
        program_id: &Pubkey, // Public key of the account the hello world program was loaded into
        accounts: &[AccountInfo], // The account to say hello to
        amount: u64,
        _instruction_data: &[u8], // Ignored, all helloworld instructions are hellos
    ) -> ProgramResult {
        // Iterating accounts is safer then indexing
        let accounts_iter = &mut accounts.iter();

        // Get the account to say hello to
        let account = next_account_info(accounts_iter)?;
        let greeter = next_account_info(accounts_iter)?;

        // The account must be owned by the program in order to modify its data
        if account.owner != program_id {
            msg!("Greeted account does not have the correct program id");
            return Err(ProgramError::IncorrectProgramId);
        }

       // Increment and store the number of times the account has been greeted
        let mut greeting_account = GreetingAccount::unpack_unchecked(&account.data.borrow())?;
        greeting_account.counter += 1;
        greeting_account.free_counter += amount;
        greeting_account.serialize(&mut &mut account.data.borrow_mut()[..])?;

        msg!("Greeted {} time(s)!", greeting_account.counter);
        msg!("Free counter: {}", greeting_account.free_counter);
        msg!("Greeted from {}!", greeter.key);

        Ok(())
    }
}


