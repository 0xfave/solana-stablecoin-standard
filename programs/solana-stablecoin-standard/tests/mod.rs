#[cfg(test)]
mod tests {
    // use solana_stablecoin_standard::ID as PROGRAM_ID;
    // use sss_compliance_hook::ID as SSS_PROGRAM_ID;
    
    use anchor_lang::{
        error::Error,
        prelude::{msg, AccountMeta},
        solana_program::{
            hash::Hash, native_token::LAMPORTS_PER_SOL, program_pack::Pack, pubkey::Pubkey,
            system_instruction,
        },
        system_program::ID as SYSTEM_PROGRAM_ID,
        AccountDeserialize, InstructionData, Key, ToAccountMetas,
    };
    use anchor_spl::{
        associated_token::{self, spl_associated_token_account},
        token_2022::spl_token_2022,
    };
    use litesvm::LiteSVM;
    use solana_instruction::Instruction;
    use solana_keypair::Keypair;
    use solana_signer::Signer;
    use solana_transaction::Transaction;
    use spl_token_2022::ID as TOKEN_2022_ID;
    use std::path::PathBuf;
    
    use solana_stablecoin_standard::ID as PROGRAM_ID;
    const SSS_PROGRAM_ID: Pubkey = sss_compliance_hook::ID;
    
    pub struct ReusableData {
        pub svm: LiteSVM,
        pub payer: Keypair,
        pub mint: Keypair,
        pub mint_authority: Keypair,
        pub token_program: Pubkey,
        pub system_program: Pubkey,
    }
    
    #[derive(Debug)]
    struct StablecoinConfig {
        master_authority: Pubkey,
        mint: Pubkey,
        preset: u8,
        paused: bool,
        supply_cap: Option<u64>,
        transfer_hook_program: Option<Pubkey>,
        decimals: u8,
        bump: u8,
        pending_master_authority: Option<Pubkey>,
    }
    
    pub fn setup() -> ReusableData {
        let mut svm = LiteSVM::new();
        let payer = Keypair::new();
        let mint = Keypair::new();
        let mint_authority = Keypair::new();
    
        svm.airdrop(&payer.pubkey(), 10 * LAMPORTS_PER_SOL)
            .expect("Failed to airdrop SOL to payer");
    
        let program_so_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../target/deploy/solana_stablecoin_standard.so");
        let program_data = std::fs::read(program_so_path).expect("Failed to read program SO file");
        svm.add_program(PROGRAM_ID, &program_data);
    
        let sss_compliance_hook_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../target/deploy/sss_compliance_hook.so");
        let sss_compliance_hook_data = std::fs::read(sss_compliance_hook_path).expect("Failed to read program SO file");
        svm.add_program(SSS_PROGRAM_ID, &sss_compliance_hook_data);
    
        let token_program = TOKEN_2022_ID;
        let system_program = SYSTEM_PROGRAM_ID;
    
        ReusableData {
            svm,
            payer,
            mint,
            mint_authority,
            token_program,
            system_program,
        }
    }
    
    pub fn create_mint(
        svm: &mut LiteSVM,
        payer: &Keypair,
        mint: &Keypair,
        mint_authority: &Pubkey,
        decimals: u8,
    ) {
        let token_program = TOKEN_2022_ID;
    
        let mint_space = 82;
        let lamports = svm.minimum_balance_for_rent_exemption(mint_space);
    
        let create_mint_ix = system_instruction::create_account(
            &payer.pubkey(),
            &mint.pubkey(),
            lamports,
            mint_space as u64,
            &token_program,
        );
    
        let initialize_mint_ix = spl_token_2022::instruction::initialize_mint(
            &token_program,
            &mint.pubkey(),
            mint_authority,
            None,
            decimals,
        )
        .unwrap();
    
        let blockhash = svm.latest_blockhash();
    
        let tx = Transaction::new_signed_with_payer(
            &[create_mint_ix, initialize_mint_ix],
            Some(&payer.pubkey()),
            &[payer, mint],
            blockhash,
        );
    
        svm.send_transaction(tx).expect("Failed to create mint");
    }
    
    pub fn initialize(
        svm: &mut LiteSVM,
        payer: &Keypair,
        authority: &Keypair,
        mint: &Pubkey,
        preset: u8,
        supply_cap: Option<u64>,
        decimals: u8,
    ) -> Pubkey {
        let (config_pda, _) =
            Pubkey::find_program_address(&[b"stablecoin", mint.as_ref()], &PROGRAM_ID);
    
        let token_program = TOKEN_2022_ID;
    
        let accounts = vec![
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ];
    
        let mut data = vec![0u8; 8];
        data.push(preset);
        if let Some(cap) = supply_cap {
            data.push(1);
            data.extend_from_slice(&cap.to_le_bytes());
        } else {
            data.push(0);
        }
        data.push(decimals);
    
        let blockhash = svm.latest_blockhash();
    
        let tx = Transaction::new_signed_with_payer(
            &[Instruction {
                program_id: PROGRAM_ID,
                accounts,
                data,
            }],
            Some(&payer.pubkey()),
            &[payer, authority],
            blockhash,
        );
    
        svm.send_transaction(tx).expect("Failed to initialize");
    
        config_pda
    }
    
    #[test]
    fn test_initialize() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;
    
        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);
    
        let config = initialize(
            svm,
            payer,
            mint_authority,
            &mint.pubkey(),
            1,
            Some(1_000_000_000_000),
            6,
        );
    
        let config_account = svm.get_account(&config).unwrap();
        let config_data = solana_stablecoin_standard::state::StablecoinConfig::try_deserialize(&mut config_account.data.as_ref()).unwrap();
    
        assert_eq!(config_data.mint, mint.pubkey());
        assert_eq!(config_data.master_authority, mint_authority.pubkey());
        assert_eq!(config_data.preset, 1);
        assert_eq!(config_data.paused, false);
        assert_eq!(config_data.supply_cap, Some(1_000_000_000_000));
        assert_eq!(config_data.decimals, 6);
    }
}