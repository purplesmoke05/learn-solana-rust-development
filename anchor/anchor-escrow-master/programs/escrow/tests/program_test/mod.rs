use anchor_lang::solana_program::program_pack::Pack;
use anchor_lang::{prelude::*, InstructionData};
use assert_matches::assert_matches;
use bincode::deserialize;
use solana_program_test::{processor, ProgramTest, ProgramTestContext};
use solana_sdk::account::ReadableAccount;
use solana_sdk::{
    instruction::{Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction, sysvar,
    transaction::{Transaction, TransactionError},
    account::Account,
};
use std::mem::size_of;

pub struct EscrowProgramTestConfig {
    pub compute_limit: u64,
    pub num_users: usize,
    pub num_mints: usize,
}

impl EscrowProgramTestConfig {
    #[allow(dead_code)]
    pub fn default() -> Self {
        EscrowProgramTestConfig {
            compute_limit: 200_000,
            num_users: 2,
            num_mints: 16
        }
    }
    #[allow(dead_code)]
    pub fn default_two_mints() -> Self {
        EscrowProgramTestConfig { num_mints: 2, ..Self::default() }
    }
}

pub trait AddPacked {
    fn add_packable_account<T: Pack>(
        &mut self,
        pubkey: Pubkey,
        amount: u64,
        data: &T,
        owner: &Pubkey,
    );
}

impl AddPacked for ProgramTest {
    fn add_packable_account<T: Pack>(
        &mut self,
        pubkey: Pubkey,
        amount: u64,
        data: &T,
        owner: &Pubkey,
    ) {
        let mut account = solana_sdk::account::Account::new(amount, T::get_packed_len(), owner);
        data.pack_into_slice(&mut account.data);
        self.add_account(pubkey, account);
    }
}
pub struct EscrowProgramTest {
    pub context: ProgramTestContext,
    pub rent: Rent,
    pub program_id: Pubkey,
    // pub num_mints: usize,
    // pub quote_index: usize,
    // pub quote_mint: MintCookie,
    // pub mints: Vec<MintCookie>,
    // pub num_users: usize,
    // pub users: Vec<Keypair>,
    // pub vaults: Vec<Pubkey>,
    // pub vault_bumps: Vec<u8>,
    // pub token_accounts: Vec<Pubkey>, // user x mint
}

impl EscrowProgramTest {
    pub async fn start_new() -> Self {
        let pt = ProgramTest::new("escrow", escrow::ID, processor!(escrow::entry));
        let mut context = pt.start_with_context().await;
        let rent = context.banks_client.get_rent().await.unwrap();

        Self {
            context,
            rent,
            program_id: escrow::ID,
        }
    }

    pub async fn process_tx_and_assert_ok(
        &mut self,
        instructions: &[Instruction],
        signers: &[&Keypair],
    ) {
        let mut all_signers = vec![&self.context.payer];
        all_signers.extend_from_slice(signers);

        let tx = Transaction::new_signed_with_payer(
            &instructions,
            Some(&self.context.payer.pubkey()),
            &all_signers,
            self.context.last_blockhash,
        );

        assert_matches!(
            self.context.banks_client.process_transaction(tx).await,
            Ok(())
        );
    }

    pub async fn process_tx_and_assert_err(
        &mut self,
        instructions: &[Instruction],
        signers: &[&Keypair],
        transaction_error: TransactionError,
    ) {
        let mut all_signers = vec![&self.context.payer];
        all_signers.extend_from_slice(signers);

        let tx = Transaction::new_signed_with_payer(
            &instructions,
            Some(&self.context.payer.pubkey()),
            &all_signers,
            self.context.last_blockhash,
        );

        assert_eq!(
            transaction_error,
            self.context
                .banks_client
                .process_transaction(tx)
                .await
                .unwrap_err()
                .unwrap(),
        );
    }

    pub async fn get_clock(&mut self) -> Clock {
        deserialize::<Clock>(
            &self
                .context
                .banks_client
                .get_account(sysvar::clock::ID)
                .await
                .unwrap()
                .unwrap()
                .data,
        )
        .unwrap()
    }
}

pub async fn initialize_mint(
    mint_keypair: &Keypair,
    decimals: u8,
    escrow_program_test: &mut EscrowProgramTest,
) {
    let mint_rent_exempt_threshold = escrow_program_test
        .context
        .banks_client
        .get_rent()
        .await
        .unwrap()
        .minimum_balance(spl_token::state::Mint::LEN);

        escrow_program_test
        .process_tx_and_assert_ok(
            &[
                system_instruction::create_account(
                    &escrow_program_test.context.payer.pubkey(),
                    &mint_keypair.pubkey(),
                    mint_rent_exempt_threshold,
                    spl_token::state::Mint::LEN as u64,
                    &spl_token::id(),
                ),
                spl_token::instruction::initialize_mint(
                    &spl_token::id(),
                    &mint_keypair.pubkey(),
                    &escrow_program_test.context.payer.pubkey(),
                    None,
                    decimals,
                )
                .unwrap(),
            ],
            &[mint_keypair],
        )
        .await;
}

pub async fn initialize_ata(
    user: &Pubkey,
    mint: &Pubkey,
    escrow_program_test: &mut EscrowProgramTest,
) -> Pubkey {
    escrow_program_test
        .process_tx_and_assert_ok(
            &[
                spl_associated_token_account::create_associated_token_account(
                    &escrow_program_test.context.payer.pubkey(),
                    user,
                    mint,
                ),
            ],
            &[],
        )
        .await;
    spl_associated_token_account::get_associated_token_address(user, mint)
}

// To simplify, the payer is mint authority of all mints
pub async fn mint_some(
    token_account: &Pubkey,
    mint: &Pubkey,
    escrow_program_test: &mut EscrowProgramTest,
    amount: u64,
) {
    escrow_program_test
        .process_tx_and_assert_ok(
            &[spl_token::instruction::mint_to(
                &spl_token::id(),
                mint,
                &token_account,
                &escrow_program_test.context.payer.pubkey(),
                &[],
                amount,
            )
            .unwrap()],
            &[],
        )
        .await;
}

pub async fn get_token_balance(pubkey: Pubkey, escrow_program_test: &mut EscrowProgramTest) -> u64 {
    let token: Account = escrow_program_test.context.banks_client.get_account(pubkey).await.unwrap().unwrap();

    spl_token::state::Account::unpack(&token.data[..])
        .unwrap()
        .amount
}

pub async fn get_lamport_balance(address: Pubkey, escrow_program_test: &mut EscrowProgramTest) -> u64 {
    escrow_program_test.context.banks_client.get_account(address).await.unwrap().unwrap().lamports()
}

pub async fn get_rent_minimum_balance(len: usize, escrow_program_test: &mut EscrowProgramTest) -> u64 {
    let rent_exempt_threshold = escrow_program_test
        .context
        .banks_client
        .get_rent()
        .await
        .unwrap()
        .minimum_balance(len);
    return rent_exempt_threshold
}

pub async fn airdrop(receiver: &Pubkey, amount: u64, escrow_program_test: &mut EscrowProgramTest) {
    let rent_exempt_threshold = escrow_program_test
        .context
        .banks_client
        .get_rent()
        .await
        .unwrap()
        .minimum_balance(size_of::<Account>());

    let tx = Transaction::new_signed_with_payer(
        &[system_instruction::transfer(
            &escrow_program_test.context.payer.pubkey(),
            receiver,
            rent_exempt_threshold + amount,
        )],
        Some(&escrow_program_test.context.payer.pubkey()),
        &[&escrow_program_test.context.payer],
        escrow_program_test.context.last_blockhash,
    );

    escrow_program_test.context.banks_client.process_transaction(tx).await.unwrap();
}
