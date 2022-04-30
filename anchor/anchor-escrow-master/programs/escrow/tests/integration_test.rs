#[cfg(test)]
mod test {
    use escrow::{self, entry};
    use std::{vec};
    use anchor_lang::prelude::*;
    use solana_sdk::{instruction::Instruction, commitment_config::CommitmentLevel, transport::Result as SolanaResult, program_option::COption};
    use solana_program::{system_program, program_pack::Pack};
    use solana_program_test::*;
    use {
        anchor_client::{
            solana_sdk::{
                account::Account,
                signature::Keypair,
                commitment_config::CommitmentConfig,
                signature::Signer, transaction::Transaction,
                sysvar
            },
            Client, Cluster
        },
        std::rc::Rc,
    };
    use spl_token::{
        state::{Mint},
    };

    const RUST_LOG_DEFAULT: &str = "solana_rbpf::vm=info,\
    solana_program_runtime::stable_log=debug,\
    solana_runtime::message_processor=debug,\
    solana_runtime::system_instruction_processor=info,\
    solana_program_test=info,\
    solana_bpf_loader_program=debug"; // for - Program ... consumed 5857 of 200000 compute units

    #[derive(Copy, Clone)]
    pub struct MintCookie {
        pub index: usize,
        pub decimals: u8,
        pub unit: f64,
        pub base_lot: f64,
        pub quote_lot: f64,
        pub pubkey: Option<Pubkey>,
    }

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

    pub struct EscrowProgramTest {
        pub context: ProgramTestContext,
        pub client: Client,
        pub rent: Rent,
        pub escrow_program_id: Pubkey,
        pub num_mints: usize,
        pub quote_index: usize,
        pub quote_mint: MintCookie,
        pub mints: Vec<MintCookie>,
        pub num_users: usize,
        pub users: Vec<Keypair>,
        pub token_accounts: Vec<Pubkey>, // user x mint
    }

    trait AddPacked {
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

    impl EscrowProgramTest {

        #[allow(dead_code)]
        pub async fn start_new(config: &EscrowProgramTestConfig) -> Self {
            // Predefined mints, maybe can even add symbols to them
            let mut mints: Vec<MintCookie> = vec![
                MintCookie {
                    index: 0,
                    decimals: 6,
                    unit: 10u64.pow(6) as f64,
                    base_lot: 100 as f64,
                    quote_lot: 10 as f64,
                    pubkey: None, //Some(mngo_token::ID),
                }, // symbol: "MNGO".to_string()
                MintCookie {
                    index: 1,
                    decimals: 6,
                    unit: 10u64.pow(6) as f64,
                    base_lot: 100 as f64,
                    quote_lot: 10 as f64,
                    pubkey: None, //Some(msrm_token::ID),
                }, // symbol: "MSRM".to_string()
                MintCookie {
                    index: 2,
                    decimals: 6,
                    unit: 10u64.pow(6) as f64,
                    base_lot: 100 as f64,
                    quote_lot: 10 as f64,
                    pubkey: None,
                }, // symbol: "BTC".to_string()
                MintCookie {
                    index: 3,
                    decimals: 6,
                    unit: 10u64.pow(6) as f64,
                    base_lot: 1000 as f64,
                    quote_lot: 10 as f64,
                    pubkey: None,
                }, // symbol: "ETH".to_string()
                MintCookie {
                    index: 4,
                    decimals: 9,
                    unit: 10u64.pow(9) as f64,
                    base_lot: 100000000 as f64,
                    quote_lot: 100 as f64,
                    pubkey: None,
                }, // symbol: "SOL".to_string()
                MintCookie {
                    index: 5,
                    decimals: 6,
                    unit: 10u64.pow(6) as f64,
                    base_lot: 100000 as f64,
                    quote_lot: 100 as f64,
                    pubkey: None,
                }, // symbol: "SRM".to_string()
                MintCookie {
                    index: 6,
                    decimals: 6,
                    unit: 10u64.pow(6) as f64,
                    base_lot: 100 as f64,
                    quote_lot: 10 as f64,
                    pubkey: None,
                }, // symbol: "BTC".to_string()
                MintCookie {
                    index: 7,
                    decimals: 6,
                    unit: 10u64.pow(6) as f64,
                    base_lot: 100 as f64,
                    quote_lot: 10 as f64,
                    pubkey: None,
                }, // symbol: "BTC".to_string()
                MintCookie {
                    index: 8,
                    decimals: 6,
                    unit: 10u64.pow(6) as f64,
                    base_lot: 100 as f64,
                    quote_lot: 10 as f64,
                    pubkey: None,
                }, // symbol: "BTC".to_string()
                MintCookie {
                    index: 9,
                    decimals: 6,
                    unit: 10u64.pow(6) as f64,
                    base_lot: 100 as f64,
                    quote_lot: 10 as f64,
                    pubkey: None,
                }, // symbol: "BTC".to_string()
                MintCookie {
                    index: 10,
                    decimals: 6,
                    unit: 10u64.pow(6) as f64,
                    base_lot: 100 as f64,
                    quote_lot: 10 as f64,
                    pubkey: None,
                }, // symbol: "BTC".to_string()
                MintCookie {
                    index: 11,
                    decimals: 6,
                    unit: 10u64.pow(6) as f64,
                    base_lot: 100 as f64,
                    quote_lot: 10 as f64,
                    pubkey: None,
                }, // symbol: "BTC".to_string()
                MintCookie {
                    index: 12,
                    decimals: 6,
                    unit: 10u64.pow(6) as f64,
                    base_lot: 100 as f64,
                    quote_lot: 10 as f64,
                    pubkey: None,
                }, // symbol: "BTC".to_string()
                MintCookie {
                    index: 13,
                    decimals: 6,
                    unit: 10u64.pow(6) as f64,
                    base_lot: 100 as f64,
                    quote_lot: 10 as f64,
                    pubkey: None,
                }, // symbol: "BTC".to_string()
                MintCookie {
                    index: 14,
                    decimals: 6,
                    unit: 10u64.pow(6) as f64,
                    base_lot: 100 as f64,
                    quote_lot: 10 as f64,
                    pubkey: None,
                }, // symbol: "BTC".to_string()
                MintCookie {
                    index: 15,
                    decimals: 6,
                    unit: 10u64.pow(6) as f64,
                    base_lot: 0 as f64,
                    quote_lot: 0 as f64,
                    pubkey: None,
                }, // symbol: "USDC".to_string()
            ];

            let num_mints = config.num_mints as usize;
            let quote_index = num_mints - 1;
            let mut quote_mint = mints[(mints.len() - 1) as usize];
            let num_users = config.num_users as usize;
            // Make sure that the user defined length of mint list always have the quote_mint as last
            quote_mint.index = quote_index;
            mints[quote_index] = quote_mint;

            let initializer = Keypair::new();
            let mut test = ProgramTest::new("escrow",escrow::id(), None);
            test.add_account(initializer.pubkey(), Account::default());
            let client = Client::new_with_options(
                Cluster::Debug,
                Rc::from(Keypair::new()),
                CommitmentConfig::processed(),
            );
            
            test.set_compute_max_units(config.compute_limit);
            // Supress some of the logs
            solana_logger::setup_with_default(RUST_LOG_DEFAULT);

            // Add mints in loop
            for mint_index in 0..num_mints {
                let mint_pk: Pubkey;
                if mints[mint_index].pubkey.is_none() {
                    mint_pk = Pubkey::new_unique();
                } else {
                    mint_pk = mints[mint_index].pubkey.unwrap();
                }

                test.add_packable_account(
                    mint_pk,
                    u32::MAX as u64,
                    &Mint {
                        is_initialized: true,
                        mint_authority: COption::Some(Pubkey::new_unique()),
                        decimals: mints[mint_index].decimals,
                        ..Mint::default()
                    },
                    &spl_token::id(),
                );
                mints[mint_index].pubkey = Some(mint_pk);
            }

            // add users in loop
            let mut users = Vec::new();
            let mut token_accounts = Vec::new();
            for _ in 0..num_users {
                let user_key = Keypair::new();
                test.add_account(
                    user_key.pubkey(),
                    solana_sdk::account::Account::new(
                        u32::MAX as u64,
                        0,
                        &solana_sdk::system_program::id(),
                    ),
                );
    
                // give every user 10^18 (< 2^60) of every token
                // ~~ 1 trillion in case of 6 decimals
                for mint_index in 0..num_mints {
                    let token_key = Pubkey::new_unique();
                    test.add_packable_account(
                        token_key,
                        u32::MAX as u64,
                        &spl_token::state::Account {
                            mint: mints[mint_index].pubkey.unwrap(),
                            owner: user_key.pubkey(),
                            amount: 1_000_000_000_000_000_000,
                            state: spl_token::state::AccountState::Initialized,
                            ..spl_token::state::Account::default()
                        },
                        &spl_token::id(),
                    );
    
                    token_accounts.push(token_key);
                }
                users.push(user_key);
            }

            let mut context = test.start_with_context().await;
            let rent = context.banks_client.get_rent().await.unwrap();
            mints = mints[..num_mints].to_vec();

            Self {
                context,
                client,
                rent,
                escrow_program_id: escrow::id(),
                num_mints,
                quote_index,
                quote_mint,
                mints,
                num_users,
                users,
                token_accounts,
            }
        }
        #[allow(dead_code)]
        pub async fn process_transaction(
            &mut self,
            instructions: &[Instruction],
            signers: Option<&[&Keypair]>,
        ) -> SolanaResult<()> {
            let mut transaction =
                Transaction::new_with_payer(&instructions, Some(&self.context.payer.pubkey()));

            let mut all_signers = vec![&self.context.payer];

            if let Some(signers) = signers {
                all_signers.extend_from_slice(signers);
            }

            transaction.sign(&all_signers, self.context.last_blockhash);

            self.context
                .banks_client
                .process_transaction_with_commitment(transaction, CommitmentLevel::Processed)
                .await
        }
        #[allow(dead_code)]
        pub fn with_mint(&mut self, mint_index: usize) -> MintCookie {
            return self.mints[mint_index];
        }

        #[allow(dead_code)]
        pub fn with_user_token_account(&mut self, user_index: usize, mint_index: usize) -> Pubkey {
            return self.token_accounts[(user_index * self.num_mints) + mint_index];
        }
    }



    pub fn initialize_escrow_scenario(
        test: &mut EscrowProgramTest,
        deposit_token_index: usize,
        deposit_amount: usize,
        deposit_user_index: usize,
        receive_token_index: usize,
        receive_amount: usize,
        receive_user_index: usize
    ) {
        let deposit_token_mint = test.with_mint(deposit_token_index);
        let initializer_deposit_token_account = test.with_user_token_account(deposit_user_index, deposit_token_index);
        let initializer_receive_token_account = test.with_user_token_account(deposit_user_index, receive_token_index);
        
        let deposit_user = test.users[deposit_user_index].pubkey();
        let escrow_account = Keypair::new();
        let (vault_account_pda, _bump_seed) = Pubkey::find_program_address(&[b"escrow"], &Pubkey::new_unique());
        
        let program = test.client.program(escrow::id());
        let ix = program
        .request()
        .accounts(escrow::accounts::InitializeEscrow {
            initializer: deposit_user,
            mint: deposit_token_mint.pubkey.unwrap(),
            vault_account: vault_account_pda,
            initializer_deposit_token_account: initializer_deposit_token_account.key(),
            initializer_receive_token_account: initializer_receive_token_account.key(),
            escrow_account: escrow_account.pubkey(),
            rent: sysvar::rent::id(),
            token_program: anchor_spl::token::ID,
            system_program: system_program::ID,
        }.to_account_metas(None))
        .args(escrow::instruction::InitializeEscrow{
            _vault_account_bump: _bump_seed,
            initializer_amount: deposit_amount as u64,
            taker_amount: receive_amount as u64,
        })
        .instructions().unwrap().pop().unwrap();

        let ixs =  &[ix];
        test.process_transaction(ixs, None);
    }
    
    #[tokio::test]
    #[cfg(test)]
    async fn test_escrow_initialize() {
        let config = EscrowProgramTestConfig{num_users: 2,..EscrowProgramTestConfig::default()};
        let mut test = EscrowProgramTest::start_new(&config).await;
        initialize_escrow_scenario(&mut test,0, 10, 0, 1, 100, 1)
        
    }
}