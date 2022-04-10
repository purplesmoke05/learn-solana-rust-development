// program logic

use solana_program::{
    account_info::{ next_account_info, AccountInfo },
    entrypoint::ProgramResult,
    program_error::ProgramError,
    msg,
    pubkey::Pubkey,
    program::{invoke},
    program_pack::{Pack, IsInitialized },
    sysvar::{ rent::Rent, Sysvar },
};
use spl_token::solana_program::program::invoke_signed;
use spl_token::state::Account as TokenAccount;

use crate::{instruction::EscrowInstruction, error::EscrowError, state::Escrow};

pub struct Processor;
impl Processor {
    pub fn process(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8]
    ) -> ProgramResult {
        // instructionデータ(バイト列)をEscrowInstructionに復元
        let instruction = EscrowInstruction::unpack(instruction_data)?;

        match instruction {
            EscrowInstruction::InitEscrow { amount } => {
                msg!("Instruction: InitEscrow");
                Self::process_init_escrow(accounts, amount, program_id)
            },
            EscrowInstruction::Exchange { amount } => {
                msg!("Instruction: Exchange");
                Self::process_exchange(accounts, amount, program_id)
            }
        }
    }

    fn process_init_escrow(
        accounts: &[AccountInfo],
        amount: u64,
        program_id: &Pubkey,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        /// 0. `[signer]`   The account of the person initializing the escrow
        let initializer = next_account_info(account_info_iter)?;
        
        if !initializer.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        /// 1. `[writable]` Temporary token account that should be created prior to this instruction and owned by the initializer
        let temp_token_account = next_account_info(account_info_iter)?;

        /// 2. `[]`         The initializer's token account for the token they will receive should the trade go through
        let token_to_receive_account = next_account_info(account_info_iter)?;
        if *token_to_receive_account.owner != spl_token::id() {
            return Err(ProgramError::IncorrectProgramId);
        }

        /// 3. `[writable]` The escrow account, it will hold all necessary info about the trade
        let escrow_account = next_account_info(account_info_iter)?;
        /// 4. `[]`         The rent sysvar
        let rent = &Rent::from_account_info(
            next_account_info(account_info_iter)?
        )?;
        // 新規作成したEscrow情報を保持するアカウントが、家賃免除とされるlamports以上を保有していなければ、リバートする。
        if !rent.is_exempt(escrow_account.lamports(), escrow_account.data_len()) {
            return Err(EscrowError::NotRentExempt.into());
        }
        // Escrow情報を保持するアカウントアドレスをEscrow型にキャストする。
        let mut escrow_info = Escrow::unpack_unchecked(&escrow_account.data.borrow())?;
        // Escrowアカウントが初期化済みであればリバートする。
        if escrow_info.is_initialized() {
            return Err(ProgramError::AccountAlreadyInitialized);
        }
        // Escrowアカウントの各属性に値を書き込む
        // 初期化フラグを立てる
        escrow_info.is_initialized = true;
        // Escrowアカウントを初期化した張本人の公開鍵を格納する。
        escrow_info.initializer_pubkey = *initializer.key;
        // Escrowアカウントがテイカーに向けてトークンを送付する際に使用するアカウントの公開鍵を格納する。
        escrow_info.temp_token_account_pubkey = *temp_token_account.key;
        // Escrowアカウントからトークンを受け取る一時アカウントの公開鍵を格納する。
        escrow_info.initializer_token_to_receive_account_pubkey = *token_to_receive_account.key;
        // 初期化した張本人が要求するトークン数量を格納する。
        escrow_info.expected_amount = amount;

        // 再格納する。（アカウントに情報を書き込む）
        Escrow::pack(
            escrow_info,
            &mut escrow_account.try_borrow_mut_data()?
        )?;

        // PDAアカウント＝プログラム派生アカウント
        // 固定シード値を用いてPDAを生成する。
        let (pda, _bump_seed) = Pubkey::find_program_address(
            &[b"escrow"],
            program_id
        );

        /// 5. `[]`         The token program
        let token_program = next_account_info(account_info_iter)?;

        // Escrowアカウントがテイカーに向けてトークンを送付する際に使用するアカウントの所有者をPDAに変更する。
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

    fn process_exchange(
        accounts: &[AccountInfo],
        amount_expected_by_taker: u64,
        program_id: &Pubkey,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        // テイカーのアカウント情報を格納する
        let taker = next_account_info(account_info_iter)?;

        // テイカーが署名者本人でなければリバートする
        if !taker.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        // テイカーがトークンを送る際に使うアカウントを格納する
        let takers_sending_token_account = next_account_info(account_info_iter)?;

        // テイカーがトークンを受け取るアカウントを格納する
        let takers_token_to_receive_account = next_account_info(account_info_iter)?;

        // PDAに所有権を移譲されたアカウントを格納する
        let pdas_temp_token_account = next_account_info(account_info_iter)?;
        // TokenAccountにキャストする
        let pdas_temp_token_account_info =
            TokenAccount::unpack(&pdas_temp_token_account.try_borrow_data()?)?;

        let (pda, bump_seed) = Pubkey::find_program_address(&[b"escrow"], program_id);

        if amount_expected_by_taker != pdas_temp_token_account_info.amount {
            return Err(EscrowError::ExpectedAmountMismatch.into());
        }

        let initializers_main_account = next_account_info(account_info_iter)?;
        let initializers_token_to_receive_account = next_account_info(account_info_iter)?;
        let escrow_account = next_account_info(account_info_iter)?;

        let escrow_info = Escrow::unpack(&escrow_account.try_borrow_data()?)?;

        if escrow_info.temp_token_account_pubkey != *pdas_temp_token_account.key {
            return Err(ProgramError::InvalidAccountData);
        }

        if escrow_info.initializer_pubkey != *initializers_main_account.key {
            return Err(ProgramError::InvalidAccountData);
        }

        if escrow_info.initializer_token_to_receive_account_pubkey != *initializers_token_to_receive_account.key {
            return Err(ProgramError::InvalidAccountData);
        }

        let token_program = next_account_info(account_info_iter)?;

        let transfer_to_initializer_ix = spl_token::instruction::transfer(
            token_program.key,
            takers_sending_token_account.key,
            initializers_token_to_receive_account.key,
            taker.key,
            &[&taker.key],
            escrow_info.expected_amount,
        )?;
        msg!("Calling the token program to transfer tokens to the escrow's initializer...");
        invoke(
            &transfer_to_initializer_ix,
            &[
                takers_sending_token_account.clone(),
                initializers_token_to_receive_account.clone(),
                taker.clone(),
                token_program.clone(),
            ],
        )?;
        
        
        let pda_account = next_account_info(account_info_iter)?;

        let transfer_to_taker_ix = spl_token::instruction::transfer(
            token_program.key,
            pdas_temp_token_account.key,
            takers_token_to_receive_account.key,
            &pda,
            &[&pda],
            pdas_temp_token_account_info.amount,
        )?;
        msg!("Calling the token program to transfer tokens to the taker...");
        invoke_signed(
            &transfer_to_taker_ix,
            &[
                pdas_temp_token_account.clone(),
                takers_token_to_receive_account.clone(),
                pda_account.clone(),
                token_program.clone(),
            ],
            &[&[&b"escrow"[..], &[bump_seed]]],
        )?;

        let close_pdas_temp_acc_ix = spl_token::instruction::close_account(
            token_program.key,
            pdas_temp_token_account.key,
            initializers_main_account.key,
            &pda,
            &[&pda]
        )?;
        msg!("Calling the token program to close pda's temp account...");
        invoke_signed(
            &close_pdas_temp_acc_ix,
            &[
                pdas_temp_token_account.clone(),
                initializers_main_account.clone(),
                pda_account.clone(),
                token_program.clone(),
            ],
            &[&[&b"escrow"[..], &[bump_seed]]],
        )?;

        msg!("Closing the escrow account...");
        **initializers_main_account.lamports.borrow_mut() = initializers_main_account.lamports()
            .checked_add(escrow_account.lamports())
            .ok_or(EscrowError::AmountOverflow)?;
        **escrow_account.lamports.borrow_mut() = 0;
        *escrow_account.try_borrow_mut_data()? = &mut [];

        Ok(())
    }
}