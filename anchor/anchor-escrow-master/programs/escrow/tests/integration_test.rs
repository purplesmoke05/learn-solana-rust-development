use program_test::{EscrowProgramTest, EscrowProgramTestConfig};
mod program_test;
#[cfg(test)]
mod test {
    use std::mem::size_of;
    use crate::program_test::{initialize_mint, initialize_ata, mint_some, airdrop, get_token_balance, get_lamport_balance};
    use anchor_client::solana_client::nonce_utils::get_account;
    use solana_program_test::{tokio};

    use super::*;
    use escrow;
    use anchor_lang::{prelude::*, InstructionData};
    use solana_sdk::{instruction::Instruction, system_instruction::create_account, transport::Result as SolanaResult, program_option::COption, sysvar::instructions, system_instruction};
    use solana_program::{system_program, program_pack::Pack};
    use {
        anchor_client::{
            solana_sdk::{
                signature::Keypair,
                signature::Signer,
                sysvar
            },
        },
    };

    #[tokio::test]
    #[cfg(test)]
    async fn test_escrow_initialize() {
        let mut pt = EscrowProgramTest::start_new().await;

        let escrow_account = Keypair::new();
        let escrow_initializer_keypair = Keypair::new();
        let escrow_taker_keypair = Keypair::new();
        let mint_a_keypair = Keypair::new();
        let mint_b_keypair = Keypair::new();
        let payer_pk = pt.context.payer.pubkey();

        // Mint "A" Token to escrow_initializer
        initialize_mint(&mint_a_keypair, 0, &mut pt).await;
        let initializer_a_ata = initialize_ata(
            &escrow_initializer_keypair.pubkey(),
            &mint_a_keypair.pubkey(),
            &mut pt,
        ).await;
        mint_some(
            &initializer_a_ata,
            &mint_a_keypair.pubkey(),
            &mut pt,
            500000,
        )
        .await;
        let balance = get_token_balance(initializer_a_ata, &mut pt).await;
        assert!(balance == 500000);

        initialize_mint(&mint_b_keypair, 3, &mut pt).await;
        let initializer_b_ata = initialize_ata(
            &escrow_initializer_keypair.pubkey(),
            &mint_b_keypair.pubkey(),
            &mut pt,
        ).await;
        
        // Create Vault PDA
        let (vault_pda, pda_bump) = Pubkey::find_program_address(
            &[b"token-seed".as_ref()],
            &pt.program_id,
        );

        let escrow_rent_exempt_threshold = pt
        .context
        .banks_client
        .get_rent()
        .await
        .unwrap()
        .minimum_balance(8 + size_of::<escrow::EscrowAccount>());

        airdrop(&escrow_initializer_keypair.pubkey(), 1_000_000_000, &mut pt).await;
        pt.process_tx_and_assert_ok(&[
            system_instruction::create_account(
                &escrow_initializer_keypair.pubkey(),
                &escrow_account.pubkey(),
                escrow_rent_exempt_threshold,
                8 + size_of::<escrow::EscrowAccount>() as u64,
                &pt.program_id,
            ),
            Instruction{
                program_id: pt.program_id,
                accounts: escrow::accounts::InitializeEscrow {
                    initializer: escrow_initializer_keypair.pubkey(),
                    mint: mint_a_keypair.pubkey() ,
                    vault_account: vault_pda,
                    initializer_deposit_token_account: initializer_a_ata,
                    initializer_receive_token_account: initializer_b_ata,
                    escrow_account: escrow_account.pubkey(),
                    system_program: system_program::id(),
                    rent: sysvar::rent::ID,
                    token_program: spl_token::id(),
                }.to_account_metas(None),
                data: escrow::instruction::InitializeEscrow {
                    _vault_account_bump: pda_bump,
                    initializer_amount: 100,
                    taker_amount: 1000,
                }.data()
            }
        ], &[&escrow_initializer_keypair, &escrow_account]).await;

    }
}