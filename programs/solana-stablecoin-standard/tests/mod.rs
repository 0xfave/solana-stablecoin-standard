#[cfg(test)]
mod tests {
    use anchor_lang::{
        prelude::AccountMeta,
        solana_program::{hash::hash, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, system_instruction},
        system_program::ID as SYSTEM_PROGRAM_ID,
        AccountDeserialize, AnchorSerialize,
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
    use std::{path::PathBuf, str::FromStr};

    use solana_stablecoin_standard::ID as PROGRAM_ID;
    const SSS_PROGRAM_ID: Pubkey = sss_compliance_hook::ID;

    #[derive(AnchorSerialize)]
    struct MintTokenData {
        amount: u64,
        decimals: u8,
    }

    fn serialize_with_discriminator(discriminator: &[u8; 8], args: &[u8]) -> Vec<u8> {
        let mut data = discriminator.to_vec();
        data.extend_from_slice(args);
        data
    }

    fn compute_instruction_discriminator(name: &str) -> [u8; 8] {
        let preimage = format!("global:{}", name);
        let hash_result = hash(preimage.as_bytes());
        let mut discriminator = [0u8; 8];
        discriminator.copy_from_slice(&hash_result.to_bytes()[..8]);
        discriminator
    }

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

        // println!("[DEBUG] Setup - Payer: {}", payer.pubkey());

        svm.airdrop(&payer.pubkey(), 10 * LAMPORTS_PER_SOL).expect("Failed to airdrop SOL to payer");

        // Also airdrop to mint_authority since it's the payer for the config account
        svm.airdrop(&mint_authority.pubkey(), 10 * LAMPORTS_PER_SOL).expect("Failed to airdrop SOL to mint_authority");

        let program_so_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/deploy/solana_stablecoin_standard.so");
        // println!("[DEBUG] Loading program from: {:?}", program_so_path);
        let program_data = std::fs::read(program_so_path).expect("Failed to read program SO file");
        svm.add_program(PROGRAM_ID, &program_data);
        // println!("[DEBUG] Added program: {}", PROGRAM_ID);

        let sss_compliance_hook_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/deploy/sss_compliance_hook.so");
        // println!("[DEBUG] Loading SSS program from: {:?}", sss_compliance_hook_path);
        let sss_compliance_hook_data = std::fs::read(sss_compliance_hook_path).expect("Failed to read program SO file");
        svm.add_program(SSS_PROGRAM_ID, &sss_compliance_hook_data);
        // println!("[DEBUG] Added SSS program: {}", SSS_PROGRAM_ID);

        let token_program = TOKEN_2022_ID;
        let system_program = SYSTEM_PROGRAM_ID;

        ReusableData { svm, payer, mint, mint_authority, token_program, system_program }
    }

    pub fn create_mint(svm: &mut LiteSVM, payer: &Keypair, mint: &Keypair, mint_authority: &Pubkey, decimals: u8) {
        let token_program = TOKEN_2022_ID;

        // Compute the config PDA — this is deterministic from the mint pubkey
        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint.pubkey().as_ref()], &PROGRAM_ID);

        // Calculate mint space with permanent delegate extension
        let mint_space = spl_token_2022::extension::ExtensionType::try_calculate_account_len::<
            spl_token_2022::state::Mint,
        >(&[spl_token_2022::extension::ExtensionType::PermanentDelegate])
        .unwrap();
        let lamports = svm.minimum_balance_for_rent_exemption(mint_space);

        let create_mint_ix = system_instruction::create_account(
            &payer.pubkey(),
            &mint.pubkey(),
            lamports,
            mint_space as u64,
            &token_program,
        );

        // Add permanent delegate extension
        let initialize_extension_ix =
            spl_token_2022::instruction::initialize_permanent_delegate(&token_program, &mint.pubkey(), &config_pda)
                .unwrap();

        // Initialize mint (must come after extension)
        let initialize_mint_ix = spl_token_2022::instruction::initialize_mint2(
            &token_program,
            &mint.pubkey(),
            mint_authority,
            Some(mint_authority),
            decimals,
        )
        .unwrap();

        let blockhash = svm.latest_blockhash();

        let tx = Transaction::new_signed_with_payer(
            &[create_mint_ix, initialize_extension_ix, initialize_mint_ix],
            Some(&payer.pubkey()),
            &[payer, mint],
            blockhash,
        );

        let result = svm.send_transaction(tx);

        if let Err(e) = &result {
            // println!("[DEBUG] CreateMint failed: {:?}", e);
        }

        result.expect("Failed to create mint");
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
        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint.as_ref()], &PROGRAM_ID);

        let token_program = TOKEN_2022_ID;

        #[derive(anchor_lang::AnchorSerialize)]
        struct InitializeArgs {
            preset: u8,
            supply_cap: Option<u64>,
            decimals: u8,
        }

        let args = InitializeArgs { preset, supply_cap, decimals };

        let mut data = vec![0xaf, 0xaf, 0x6d, 0x1f, 0x0d, 0x98, 0x9b, 0xed];
        let args_bytes = args.try_to_vec().unwrap();
        data.extend_from_slice(&args_bytes);

        let accounts = vec![
            AccountMeta::new(config_pda, false),
            AccountMeta::new(*mint, false), // ✅ mutable
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ];

        let blockhash = svm.latest_blockhash();

        // println!("[DEBUG] Initialize tx - payer: {}, authority: {}", payer.pubkey(), authority.pubkey());
        // println!("[DEBUG] Signers: payer and authority (2 total)");
        // println!("[DEBUG] Account metas requiring signatures: authority (is_signer=true)");
        // println!("[DEBUG] Blockhash: {}", blockhash);
        // println!("[DEBUG] Instruction data: {:?}", data);

        let tx = Transaction::new_signed_with_payer(
            &[Instruction { program_id: PROGRAM_ID, accounts, data }],
            Some(&payer.pubkey()),
            &[payer, authority],
            blockhash,
        );

        // println!("[DEBUG] Transaction created");
        // println!("[DEBUG] Sending initialize transaction...");

        let result = svm.send_transaction(tx);

        if let Err(e) = &result {
            println!("[DEBUG] Transaction failed: {:?}", e);
        }

        result.expect("Failed to initialize");

        config_pda
    }

    #[test]
    fn test_initialize_sss1_preset0() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // SSS-1: preset = 0, no transfer hook
        let config = initialize(
            svm,
            payer,
            mint_authority,
            &mint.pubkey(),
            0, // preset = 0 for SSS-1
            Some(1_000_000_000_000),
            6,
        );

        let config_account = svm.get_account(&config).unwrap();
        let config_data =
            solana_stablecoin_standard::state::StablecoinConfig::try_deserialize(&mut config_account.data.as_ref())
                .unwrap();

        assert_eq!(config_data.mint, mint.pubkey());
        assert_eq!(config_data.master_authority, mint_authority.pubkey());
        assert_eq!(config_data.preset, 0); // SSS-1 preset
        assert_eq!(config_data.paused, false);
        assert_eq!(config_data.supply_cap, Some(1_000_000_000_000));
        assert_eq!(config_data.decimals, 6);
        assert_eq!(config_data.transfer_hook_program, None); // No transfer hook
    }

    fn create_token_account(svm: &mut LiteSVM, payer: &Keypair, mint: &Pubkey) -> Pubkey {
        let ata =
            associated_token::get_associated_token_address_with_program_id(&payer.pubkey(), mint, &spl_token_2022::ID);

        let create_ata_ix = spl_associated_token_account::instruction::create_associated_token_account(
            &payer.pubkey(),
            &payer.pubkey(),
            mint,
            &spl_token_2022::ID,
        );

        let tx = Transaction::new_signed_with_payer(
            &[create_ata_ix],
            Some(&payer.pubkey()),
            &[payer],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx).unwrap();

        ata
    }

    fn create_token_account_for_owner(svm: &mut LiteSVM, payer: &Keypair, owner: &Keypair, mint: &Pubkey) -> Pubkey {
        let ata =
            associated_token::get_associated_token_address_with_program_id(&owner.pubkey(), mint, &spl_token_2022::ID);

        let create_ata_ix = spl_associated_token_account::instruction::create_associated_token_account(
            &payer.pubkey(),
            &owner.pubkey(),
            mint,
            &spl_token_2022::ID,
        );

        let tx = Transaction::new_signed_with_payer(
            &[create_ata_ix],
            Some(&payer.pubkey()),
            &[payer],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx).unwrap();

        ata
    }

    fn mint_tokens(
        svm: &mut LiteSVM,
        payer: &Keypair,
        mint: &Pubkey,
        mint_authority: &Keypair,
        destination: &Pubkey,
        amount: u64,
    ) {
        let token_program = TOKEN_2022_ID;

        // Use spl_token_2022 instruction helper like create_mint does
        let mint_ix = spl_token_2022::instruction::mint_to(
            &token_program,
            mint,
            destination,
            &mint_authority.pubkey(),
            &[],
            amount,
        )
        .unwrap();

        let blockhash = svm.latest_blockhash();

        let tx =
            Transaction::new_signed_with_payer(&[mint_ix], Some(&payer.pubkey()), &[payer, mint_authority], blockhash);
        svm.send_transaction(tx).expect("Failed to mint tokens");
    }

    fn program_mint(
        svm: &mut LiteSVM,
        payer: &Keypair,
        minter: &Keypair,
        mint: &Pubkey,
        destination: &Pubkey,
        amount: u64,
    ) -> Result<(), String> {
        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint.as_ref()], &PROGRAM_ID);
        let token_program = TOKEN_2022_ID;

        let discriminator = compute_instruction_discriminator("mint");
        let args = amount.try_to_vec().unwrap();
        let mut data = serialize_with_discriminator(&discriminator, &args);

        let accounts = vec![
            AccountMeta::new(config_pda, false),
            AccountMeta::new(*mint, false),
            AccountMeta::new(*destination, false),
            AccountMeta::new(minter.pubkey(), true),
            AccountMeta::new_readonly(token_program, false),
        ];

        let blockhash = svm.latest_blockhash();

        let tx = Transaction::new_signed_with_payer(
            &[Instruction { program_id: PROGRAM_ID, accounts, data }],
            Some(&payer.pubkey()),
            &[payer, minter],
            blockhash,
        );

        match svm.send_transaction(tx) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    fn freeze_account(
        svm: &mut LiteSVM,
        payer: &Keypair,
        authority: &Keypair,
        mint: &Pubkey,
        account: &Pubkey,
    ) -> Result<(), String> {
        let token_program = TOKEN_2022_ID;
        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint.as_ref()], &PROGRAM_ID);

        let discriminator = compute_instruction_discriminator("freeze_account");
        let data = serialize_with_discriminator(&discriminator, &[]);

        let accounts = vec![
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new(*account, false),
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new_readonly(token_program, false),
        ];

        let blockhash = svm.latest_blockhash();
        let tx = Transaction::new_signed_with_payer(
            &[Instruction { program_id: PROGRAM_ID, accounts, data }],
            Some(&payer.pubkey()),
            &[payer, authority],
            blockhash,
        );

        match svm.send_transaction(tx) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    fn thaw_account(
        svm: &mut LiteSVM,
        payer: &Keypair,
        authority: &Keypair,
        mint: &Pubkey,
        account: &Pubkey,
    ) -> Result<(), String> {
        let token_program = TOKEN_2022_ID;
        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint.as_ref()], &PROGRAM_ID);

        let discriminator = compute_instruction_discriminator("thaw_account");
        let data = serialize_with_discriminator(&discriminator, &[]);

        let accounts = vec![
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new(*account, false),
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new_readonly(token_program, false),
        ];

        let blockhash = svm.latest_blockhash();
        let tx = Transaction::new_signed_with_payer(
            &[Instruction { program_id: PROGRAM_ID, accounts, data }],
            Some(&payer.pubkey()),
            &[payer, authority],
            blockhash,
        );

        match svm.send_transaction(tx) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    fn program_burn(
        svm: &mut LiteSVM,
        payer: &Keypair,
        burner: &Keypair,
        mint: &Pubkey,
        from: &Pubkey,
        amount: u64,
    ) -> Result<(), String> {
        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint.as_ref()], &PROGRAM_ID);
        let token_program = TOKEN_2022_ID;

        let discriminator = compute_instruction_discriminator("burn");
        let args = amount.try_to_vec().unwrap();
        let mut data = serialize_with_discriminator(&discriminator, &args);

        let accounts = vec![
            AccountMeta::new(config_pda, false),
            AccountMeta::new(*mint, false),
            AccountMeta::new(*from, false),
            AccountMeta::new(burner.pubkey(), true),
            AccountMeta::new_readonly(token_program, false),
        ];

        let blockhash = svm.latest_blockhash();

        let tx = Transaction::new_signed_with_payer(
            &[Instruction { program_id: PROGRAM_ID, accounts, data }],
            Some(&payer.pubkey()),
            &[payer, burner],
            blockhash,
        );

        match svm.send_transaction(tx) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    #[test]
    fn test_initialize_sss2_preset1() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // SSS-2: preset = 1 (compliant mode)
        let config = initialize(
            svm,
            payer,
            mint_authority,
            &mint.pubkey(),
            1, // preset = 1 for SSS-2 (compliant mode)
            Some(1_000_000_000_000),
            6,
        );

        let config_account = svm.get_account(&config).unwrap();
        let config_data =
            solana_stablecoin_standard::state::StablecoinConfig::try_deserialize(&mut config_account.data.as_ref())
                .unwrap();

        assert_eq!(config_data.mint, mint.pubkey());
        assert_eq!(config_data.master_authority, mint_authority.pubkey());
        assert_eq!(config_data.preset, 1); // SSS-2 preset (compliant mode)
        assert_eq!(config_data.paused, false);
        assert_eq!(config_data.supply_cap, Some(1_000_000_000_000));
        assert_eq!(config_data.decimals, 6);
    }

    #[test]
    fn test_reinitialize_same_pda_fails() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // First initialization
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        // Try to re-initialize with same PDA - should fail
        // Use send_transaction directly to catch the error
        let mint_pubkey = mint.pubkey();
        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint_pubkey.as_ref()], &PROGRAM_ID);

        let token_program = TOKEN_2022_ID;

        #[derive(AnchorSerialize)]
        struct InitializeArgs {
            preset: u8,
            supply_cap: Option<u64>,
            decimals: u8,
        }

        let args = InitializeArgs { preset: 0, supply_cap: Some(1_000_000_000_000), decimals: 6 };

        let mut data = vec![0xaf, 0xaf, 0x6d, 0x1f, 0x0d, 0x98, 0x9b, 0xed];
        let args_bytes = args.try_to_vec().unwrap();
        data.extend_from_slice(&args_bytes);

        let accounts = vec![
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(mint_pubkey, false),
            AccountMeta::new(mint_authority.pubkey(), true),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ];

        let blockhash = svm.latest_blockhash();

        let tx = Transaction::new_signed_with_payer(
            &[Instruction { program_id: PROGRAM_ID, accounts, data }],
            Some(&payer.pubkey()),
            &[payer, mint_authority],
            blockhash,
        );

        let result = svm.send_transaction(tx);
        assert!(result.is_err(), "Re-initialization should fail");
    }

    #[test]
    fn test_initialize_with_non_signing_owner_fails() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        let mint_pubkey = mint.pubkey();

        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint_pubkey.as_ref()], &PROGRAM_ID);

        let token_program = TOKEN_2022_ID;

        #[derive(AnchorSerialize)]
        struct InitializeArgs {
            preset: u8,
            supply_cap: Option<u64>,
            decimals: u8,
        }

        let args = InitializeArgs { preset: 0, supply_cap: Some(1_000_000_000_000), decimals: 6 };

        let mut data = vec![0xaf, 0xaf, 0x6d, 0x1f, 0x0d, 0x98, 0x9b, 0xed];
        let args_bytes = args.try_to_vec().unwrap();
        data.extend_from_slice(&args_bytes);

        // Authority is NOT a signer - this should fail
        let non_signer_authority = Keypair::new();
        svm.airdrop(&non_signer_authority.pubkey(), LAMPORTS_PER_SOL).unwrap();

        let accounts = vec![
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(mint_pubkey, false),
            AccountMeta::new_readonly(non_signer_authority.pubkey(), false), // Not a signer!
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ];

        let blockhash = svm.latest_blockhash();

        let tx = Transaction::new_signed_with_payer(
            &[Instruction { program_id: PROGRAM_ID, accounts, data }],
            Some(&payer.pubkey()),
            &[payer], // Only payer signs, not authority
            blockhash,
        );

        let result = svm.send_transaction(tx);
        assert!(result.is_err(), "Initialization with non-signing owner should fail");
    }

    fn update_paused(
        svm: &mut LiteSVM,
        payer: &Keypair,
        authority: &Keypair,
        mint: &Pubkey,
        paused: bool,
    ) -> Result<(), String> {
        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint.as_ref()], &PROGRAM_ID);

        #[derive(AnchorSerialize)]
        struct UpdatePausedArgs {
            paused: bool,
        }

        let args = UpdatePausedArgs { paused };

        let mut data = vec![0x4e, 0xec, 0x55, 0x68, 0xa9, 0xe7, 0xcd, 0x59];
        let args_bytes = args.try_to_vec().unwrap();
        data.extend_from_slice(&args_bytes);

        let accounts = vec![AccountMeta::new(config_pda, false), AccountMeta::new(authority.pubkey(), true)];

        let blockhash = svm.latest_blockhash();

        let tx = Transaction::new_signed_with_payer(
            &[Instruction { program_id: PROGRAM_ID, accounts, data }],
            Some(&payer.pubkey()),
            &[payer, authority],
            blockhash,
        );

        match svm.send_transaction(tx) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    fn add_minter(
        svm: &mut LiteSVM,
        payer: &Keypair,
        authority: &Keypair,
        mint: &Pubkey,
        new_minter: &Pubkey,
    ) -> Result<(), String> {
        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint.as_ref()], &PROGRAM_ID);

        let discriminator = compute_instruction_discriminator("add_minter");
        let mut data = serialize_with_discriminator(&discriminator, new_minter.as_ref());

        let accounts = vec![AccountMeta::new(config_pda, false), AccountMeta::new(authority.pubkey(), true)];

        let blockhash = svm.latest_blockhash();

        let tx = Transaction::new_signed_with_payer(
            &[Instruction { program_id: PROGRAM_ID, accounts, data }],
            Some(&payer.pubkey()),
            &[payer, authority],
            blockhash,
        );

        match svm.send_transaction(tx) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    fn remove_minter(
        svm: &mut LiteSVM,
        payer: &Keypair,
        authority: &Keypair,
        mint: &Pubkey,
        minter: &Pubkey,
    ) -> Result<(), String> {
        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint.as_ref()], &PROGRAM_ID);

        let discriminator = compute_instruction_discriminator("remove_minter");
        let mut data = serialize_with_discriminator(&discriminator, minter.as_ref());

        let accounts = vec![AccountMeta::new(config_pda, false), AccountMeta::new(authority.pubkey(), true)];

        let blockhash = svm.latest_blockhash();

        let tx = Transaction::new_signed_with_payer(
            &[Instruction { program_id: PROGRAM_ID, accounts, data }],
            Some(&payer.pubkey()),
            &[payer, authority],
            blockhash,
        );

        match svm.send_transaction(tx) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    fn update_minter(
        svm: &mut LiteSVM,
        payer: &Keypair,
        authority: &Keypair,
        mint: &Pubkey,
        new_minter: &Pubkey,
    ) -> Result<(), String> {
        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint.as_ref()], &PROGRAM_ID);

        let discriminator = compute_instruction_discriminator("update_minter");
        let mut data = serialize_with_discriminator(&discriminator, new_minter.as_ref());

        let accounts = vec![AccountMeta::new(config_pda, false), AccountMeta::new(authority.pubkey(), true)];

        let blockhash = svm.latest_blockhash();

        let tx = Transaction::new_signed_with_payer(
            &[Instruction { program_id: PROGRAM_ID, accounts, data }],
            Some(&payer.pubkey()),
            &[payer, authority],
            blockhash,
        );

        match svm.send_transaction(tx) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    fn update_freezer(
        svm: &mut LiteSVM,
        payer: &Keypair,
        authority: &Keypair,
        mint: &Pubkey,
        new_freezer: &Pubkey,
    ) -> Result<(), String> {
        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint.as_ref()], &PROGRAM_ID);

        let discriminator = compute_instruction_discriminator("update_freezer");
        let mut data = serialize_with_discriminator(&discriminator, new_freezer.as_ref());

        let accounts = vec![AccountMeta::new(config_pda, false), AccountMeta::new(authority.pubkey(), true)];

        let blockhash = svm.latest_blockhash();

        let tx = Transaction::new_signed_with_payer(
            &[Instruction { program_id: PROGRAM_ID, accounts, data }],
            Some(&payer.pubkey()),
            &[payer, authority],
            blockhash,
        );

        match svm.send_transaction(tx) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    fn update_pauser(
        svm: &mut LiteSVM,
        payer: &Keypair,
        authority: &Keypair,
        mint: &Pubkey,
        new_pauser: &Pubkey,
    ) -> Result<(), String> {
        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint.as_ref()], &PROGRAM_ID);

        let discriminator = compute_instruction_discriminator("update_pauser");
        let mut data = serialize_with_discriminator(&discriminator, new_pauser.as_ref());

        let accounts = vec![AccountMeta::new(config_pda, false), AccountMeta::new(authority.pubkey(), true)];

        let blockhash = svm.latest_blockhash();

        let tx = Transaction::new_signed_with_payer(
            &[Instruction { program_id: PROGRAM_ID, accounts, data }],
            Some(&payer.pubkey()),
            &[payer, authority],
            blockhash,
        );

        match svm.send_transaction(tx) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    fn update_blacklister(
        svm: &mut LiteSVM,
        payer: &Keypair,
        authority: &Keypair,
        mint: &Pubkey,
        new_blacklister: &Pubkey,
    ) -> Result<(), String> {
        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint.as_ref()], &PROGRAM_ID);

        let discriminator = compute_instruction_discriminator("update_blacklister");
        let mut data = serialize_with_discriminator(&discriminator, new_blacklister.as_ref());

        let accounts = vec![AccountMeta::new(config_pda, false), AccountMeta::new(authority.pubkey(), true)];

        let blockhash = svm.latest_blockhash();

        let tx = Transaction::new_signed_with_payer(
            &[Instruction { program_id: PROGRAM_ID, accounts, data }],
            Some(&payer.pubkey()),
            &[payer, authority],
            blockhash,
        );

        match svm.send_transaction(tx) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    fn blacklist_add(
        svm: &mut LiteSVM,
        payer: &Keypair,
        blacklister: &Keypair,
        mint: &Pubkey,
        target: &Pubkey,
        reason: String,
    ) -> Result<(), String> {
        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint.as_ref()], &PROGRAM_ID);
        let (blacklist_entry, _) =
            Pubkey::find_program_address(&[b"blacklist", config_pda.as_ref(), target.as_ref()], &PROGRAM_ID);

        let discriminator = compute_instruction_discriminator("blacklist_add");
        let reason_bytes = reason.try_to_vec().unwrap();
        let mut data = serialize_with_discriminator(&discriminator, &reason_bytes);

        let accounts = vec![
            AccountMeta::new(blacklist_entry, false),
            AccountMeta::new(config_pda, false),
            AccountMeta::new(blacklister.pubkey(), true),
            AccountMeta::new_readonly(*target, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ];

        let blockhash = svm.latest_blockhash();

        let tx = Transaction::new_signed_with_payer(
            &[Instruction { program_id: PROGRAM_ID, accounts, data }],
            Some(&payer.pubkey()),
            &[payer, blacklister],
            blockhash,
        );

        match svm.send_transaction(tx) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    fn blacklist_remove(
        svm: &mut LiteSVM,
        payer: &Keypair,
        authority: &Keypair,
        mint: &Pubkey,
        target: &Pubkey,
    ) -> Result<(), String> {
        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint.as_ref()], &PROGRAM_ID);
        let (blacklist_entry, _) =
            Pubkey::find_program_address(&[b"blacklist", config_pda.as_ref(), target.as_ref()], &PROGRAM_ID);

        let discriminator = compute_instruction_discriminator("blacklist_remove");
        let data = serialize_with_discriminator(&discriminator, &[]);

        let accounts = vec![
            AccountMeta::new(blacklist_entry, false),
            AccountMeta::new(config_pda, false),
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new_readonly(*target, false),
            AccountMeta::new_readonly(payer.pubkey(), false),
        ];

        let blockhash = svm.latest_blockhash();

        let tx = Transaction::new_signed_with_payer(
            &[Instruction { program_id: PROGRAM_ID, accounts, data }],
            Some(&payer.pubkey()),
            &[payer, authority],
            blockhash,
        );

        match svm.send_transaction(tx) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    #[test]
    fn test_update_paused_succeeds_by_master_authority() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize with preset 0 (SSS-1)
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        // Update paused to true - should succeed
        let result = update_paused(svm, payer, mint_authority, &mint.pubkey(), true);
        assert!(result.is_ok(), "update_paused should succeed when called by master_authority");

        // Verify the config is paused
        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint.pubkey().as_ref()], &PROGRAM_ID);
        let config_account = svm.get_account(&config_pda).unwrap();
        let config_data =
            solana_stablecoin_standard::state::StablecoinConfig::try_deserialize(&mut config_account.data.as_ref())
                .unwrap();
        assert_eq!(config_data.paused, true);
    }

    #[test]
    fn test_update_paused_fails_by_non_owner() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        // Try to update paused with a non-owner - should fail
        let non_owner = Keypair::new();
        svm.airdrop(&non_owner.pubkey(), LAMPORTS_PER_SOL).unwrap();

        let result = update_paused(svm, payer, &non_owner, &mint.pubkey(), true);
        assert!(result.is_err(), "update_paused should fail when called by non-owner");
    }

    #[test]
    fn test_freeze_account_succeeds_by_master_authority() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize with preset 1 (compliant mode needed for freeze)
        let _config = initialize(
            svm,
            payer,
            mint_authority,
            &mint.pubkey(),
            1, // preset 1 for compliant mode
            Some(1_000_000_000_000),
            6,
        );

        // Create a token account and mint some tokens
        let user = Keypair::new();
        svm.airdrop(&user.pubkey(), LAMPORTS_PER_SOL).unwrap();
        let token_account = create_token_account(svm, payer, &mint.pubkey());

        // Mint tokens to the account
        // mint_tokens(svm, payer, &mint.pubkey(), mint_authority, &token_account, 1000);
        program_mint(svm, payer, mint_authority, &mint.pubkey(), &token_account, 1000).unwrap();

        // Freeze account - should succeed
        let result = freeze_account(svm, payer, mint_authority, &mint.pubkey(), &token_account);
        assert!(result.is_ok(), "freeze_account should succeed when called by master_authority");
    }

    #[test]
    fn test_freeze_account_fails_by_non_owner() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 1, Some(1_000_000_000_000), 6);

        // Create a token account
        let user = Keypair::new();
        svm.airdrop(&user.pubkey(), LAMPORTS_PER_SOL).unwrap();
        let token_account = create_token_account(svm, payer, &mint.pubkey());
        // mint_tokens(svm, payer, &mint.pubkey(), mint_authority, &token_account, 1000);
        program_mint(svm, payer, mint_authority, &mint.pubkey(), &token_account, 1000).unwrap();

        // Try to freeze with non-owner - should fail
        let non_owner = Keypair::new();
        svm.airdrop(&non_owner.pubkey(), LAMPORTS_PER_SOL).unwrap();

        let result = freeze_account(svm, payer, &non_owner, &mint.pubkey(), &token_account);
        assert!(result.is_err(), "freeze_account should fail when called by non-owner");
    }

    #[test]
    fn test_update_minter_succeeds_by_master_authority() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        // Update minter to a new address
        let new_minter = Keypair::new();
        let result = add_minter(svm, payer, mint_authority, &mint.pubkey(), &new_minter.pubkey());
        assert!(result.is_ok(), "add_minter should succeed when called by master_authority");

        // Verify the config has the new minter
        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint.pubkey().as_ref()], &PROGRAM_ID);
        let config_account = svm.get_account(&config_pda).unwrap();
        let config_data =
            solana_stablecoin_standard::state::StablecoinConfig::try_deserialize(&mut config_account.data.as_ref())
                .unwrap();
        assert!(config_data.minters.contains(&new_minter.pubkey()));
    }

    #[test]
    fn test_update_minter_fails_by_non_owner() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        // Try to add minter with non-owner - should fail
        let non_owner = Keypair::new();
        svm.airdrop(&non_owner.pubkey(), LAMPORTS_PER_SOL).unwrap();

        let new_minter = Keypair::new();
        let result = add_minter(svm, payer, &non_owner, &mint.pubkey(), &new_minter.pubkey());
        assert!(result.is_err(), "add_minter should fail when called by non-owner");
    }

    #[test]
    fn test_add_minter_fails_if_already_minter() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);
        initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        let new_minter = Keypair::new();
        add_minter(svm, payer, mint_authority, &mint.pubkey(), &new_minter.pubkey()).unwrap();

        let result = add_minter(svm, payer, mint_authority, &mint.pubkey(), &new_minter.pubkey());
        assert!(result.is_err(), "add_minter should fail if address is already a minter");
    }

    #[test]
    fn test_add_minter_fails_when_at_max_capacity() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);
        initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        // Authority is already minter (1), add 9 more to hit max of 10
        for _ in 0..9 {
            let m = Keypair::new();
            add_minter(svm, payer, mint_authority, &mint.pubkey(), &m.pubkey()).unwrap();
        }

        let config_data = get_config(svm, &mint.pubkey());
        assert_eq!(config_data.minters.len(), 10);

        // Adding an 11th should fail
        let overflow_minter = Keypair::new();
        let result = add_minter(svm, payer, mint_authority, &mint.pubkey(), &overflow_minter.pubkey());
        assert!(result.is_err(), "add_minter should fail when at max capacity (10)");
    }

    #[test]
    fn test_remove_minter_succeeds_by_master_authority() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);
        initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        let new_minter = Keypair::new();
        add_minter(svm, payer, mint_authority, &mint.pubkey(), &new_minter.pubkey()).unwrap();

        let config_data = get_config(svm, &mint.pubkey());
        assert_eq!(config_data.minters.len(), 2);

        let result = remove_minter(svm, payer, mint_authority, &mint.pubkey(), &new_minter.pubkey());
        assert!(result.is_ok(), "remove_minter should succeed when called by master_authority");

        let config_data = get_config(svm, &mint.pubkey());
        assert!(!config_data.minters.contains(&new_minter.pubkey()), "removed minter should not be in list");
        assert_eq!(config_data.minters.len(), 1);
    }

    #[test]
    fn test_remove_minter_fails_by_non_owner() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);
        initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        let non_owner = Keypair::new();
        svm.airdrop(&non_owner.pubkey(), LAMPORTS_PER_SOL).unwrap();

        let result = remove_minter(svm, payer, &non_owner, &mint.pubkey(), &mint_authority.pubkey());
        assert!(result.is_err(), "remove_minter should fail when called by non-owner");
    }

    #[test]
    fn test_remove_minter_fails_if_not_a_minter() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);
        initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        let random_key = Keypair::new();
        let result = remove_minter(svm, payer, mint_authority, &mint.pubkey(), &random_key.pubkey());
        assert!(result.is_err(), "remove_minter should fail if address is not a minter");
    }

    #[test]
    fn test_multiple_minters_can_all_mint() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);
        initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        let minter2 = Keypair::new();
        let minter3 = Keypair::new();
        svm.airdrop(&minter2.pubkey(), LAMPORTS_PER_SOL).unwrap();
        svm.airdrop(&minter3.pubkey(), LAMPORTS_PER_SOL).unwrap();

        add_minter(svm, payer, mint_authority, &mint.pubkey(), &minter2.pubkey()).unwrap();
        add_minter(svm, payer, mint_authority, &mint.pubkey(), &minter3.pubkey()).unwrap();

        // Create separate token accounts for each minter (owned by respective minter)
        let token_account1 = create_token_account_for_owner(svm, payer, mint_authority, &mint.pubkey());
        let token_account2 = create_token_account_for_owner(svm, payer, &minter2, &mint.pubkey());
        let token_account3 = create_token_account_for_owner(svm, payer, &minter3, &mint.pubkey());

        // All three minters should be able to mint to their own token accounts
        let result1 = program_mint(svm, payer, mint_authority, &mint.pubkey(), &token_account1, 100);
        if result1.is_err() {
            println!("mint_authority mint failed: {:?}", result1);
        }
        assert!(result1.is_ok(), "original authority should still be able to mint");

        let result2 = program_mint(svm, payer, &minter2, &mint.pubkey(), &token_account2, 100);
        if result2.is_err() {
            println!("minter2 mint failed: {:?}", result2);
        }
        assert!(result2.is_ok(), "minter2 should be able to mint");

        let result3 = program_mint(svm, payer, &minter3, &mint.pubkey(), &token_account3, 100);
        if result3.is_err() {
            println!("minter3 mint failed: {:?}", result3);
        }
        assert!(result3.is_ok(), "minter3 should be able to mint");
    }

    #[test]
    fn test_removed_minter_cannot_mint() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);
        initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        let minter2 = Keypair::new();
        svm.airdrop(&minter2.pubkey(), LAMPORTS_PER_SOL).unwrap();

        add_minter(svm, payer, mint_authority, &mint.pubkey(), &minter2.pubkey()).unwrap();

        // Create token account owned by minter2
        let token_account = create_token_account_for_owner(svm, payer, &minter2, &mint.pubkey());

        // Can mint before removal
        assert!(program_mint(svm, payer, &minter2, &mint.pubkey(), &token_account, 100).is_ok());

        // Remove minter2
        remove_minter(svm, payer, mint_authority, &mint.pubkey(), &minter2.pubkey()).unwrap();

        // Cannot mint after removal
        let result = program_mint(svm, payer, &minter2, &mint.pubkey(), &token_account, 100);
        assert!(result.is_err(), "removed minter should not be able to mint");
    }

    #[test]
    fn test_update_freezer_succeeds_by_master_authority() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        // Update freezer to a new address
        let new_freezer = Keypair::new();
        let result = update_freezer(svm, payer, mint_authority, &mint.pubkey(), &new_freezer.pubkey());
        assert!(result.is_ok(), "update_freezer should succeed when called by master_authority");

        // Verify the config has the new freezer
        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint.pubkey().as_ref()], &PROGRAM_ID);
        let config_account = svm.get_account(&config_pda).unwrap();
        let config_data =
            solana_stablecoin_standard::state::StablecoinConfig::try_deserialize(&mut config_account.data.as_ref())
                .unwrap();
        assert_eq!(config_data.freezer, new_freezer.pubkey());
    }

    #[test]
    fn test_update_freezer_fails_by_non_owner() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        // Try to update freezer with non-owner - should fail
        let non_owner = Keypair::new();
        svm.airdrop(&non_owner.pubkey(), LAMPORTS_PER_SOL).unwrap();

        let new_freezer = Keypair::new();
        let result = update_freezer(svm, payer, &non_owner, &mint.pubkey(), &new_freezer.pubkey());
        assert!(result.is_err(), "update_freezer should fail when called by non-owner");
    }

    #[test]
    fn test_update_pauser_succeeds_by_master_authority() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        // Update pauser to a new address
        let new_pauser = Keypair::new();
        let result = update_pauser(svm, payer, mint_authority, &mint.pubkey(), &new_pauser.pubkey());
        assert!(result.is_ok(), "update_pauser should succeed when called by master_authority");

        // Verify the config has the new pauser
        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint.pubkey().as_ref()], &PROGRAM_ID);
        let config_account = svm.get_account(&config_pda).unwrap();
        let config_data =
            solana_stablecoin_standard::state::StablecoinConfig::try_deserialize(&mut config_account.data.as_ref())
                .unwrap();
        assert_eq!(config_data.pauser, new_pauser.pubkey());
    }

    #[test]
    fn test_update_pauser_fails_by_non_owner() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        // Try to update pauser with non-owner - should fail
        let non_owner = Keypair::new();
        svm.airdrop(&non_owner.pubkey(), LAMPORTS_PER_SOL).unwrap();

        let new_pauser = Keypair::new();
        let result = update_pauser(svm, payer, &non_owner, &mint.pubkey(), &new_pauser.pubkey());
        assert!(result.is_err(), "update_pauser should fail when called by non-owner");
    }

    #[test]
    fn test_update_blacklister_succeeds_sss2() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize with preset 1 (SSS-2 / compliant mode)
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 1, Some(1_000_000_000_000), 6);

        // Update blacklister to a new address
        let new_blacklister = Keypair::new();
        let result = update_blacklister(svm, payer, mint_authority, &mint.pubkey(), &new_blacklister.pubkey());
        assert!(result.is_ok(), "update_blacklister should succeed when called by master_authority in SSS-2");

        // Verify the config has the new blacklister
        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint.pubkey().as_ref()], &PROGRAM_ID);
        let config_account = svm.get_account(&config_pda).unwrap();
        let config_data =
            solana_stablecoin_standard::state::StablecoinConfig::try_deserialize(&mut config_account.data.as_ref())
                .unwrap();
        assert_eq!(config_data.blacklister, new_blacklister.pubkey());
    }

    #[test]
    fn test_update_blacklister_fails_sss1() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize with preset 0 (SSS-1)
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        // Try to update blacklister in SSS-1 - should fail
        let new_blacklister = Keypair::new();
        let result = update_blacklister(svm, payer, mint_authority, &mint.pubkey(), &new_blacklister.pubkey());
        assert!(result.is_err(), "update_blacklister should fail in SSS-1");
    }

    #[test]
    fn test_mint_succeeds_when_caller_is_minter() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        // Create token account
        let user = Keypair::new();
        svm.airdrop(&user.pubkey(), LAMPORTS_PER_SOL).unwrap();
        let token_account = create_token_account(svm, payer, &mint.pubkey());

        // Mint tokens using program instruction (minter is mint_authority)
        let result = program_mint(svm, payer, mint_authority, &mint.pubkey(), &token_account, 1000);
        assert!(result.is_ok(), "mint should succeed when called by minter");
    }

    #[test]
    fn test_mint_fails_if_caller_is_not_minter() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        // Create token account
        let user = Keypair::new();
        svm.airdrop(&user.pubkey(), LAMPORTS_PER_SOL).unwrap();
        let token_account = create_token_account(svm, payer, &mint.pubkey());

        // Try to mint with non-minter - should fail
        let non_minter = Keypair::new();
        svm.airdrop(&non_minter.pubkey(), LAMPORTS_PER_SOL).unwrap();

        let result = program_mint(svm, payer, &non_minter, &mint.pubkey(), &token_account, 1000);
        assert!(result.is_err(), "mint should fail when called by non-minter");
    }

    #[test]
    fn test_mint_fails_when_paused() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        // Pause the mint
        update_paused(svm, payer, mint_authority, &mint.pubkey(), true).unwrap();

        // Create token account
        let user = Keypair::new();
        svm.airdrop(&user.pubkey(), LAMPORTS_PER_SOL).unwrap();
        let token_account = create_token_account(svm, payer, &mint.pubkey());

        // Try to mint when paused - should fail
        let result = program_mint(svm, payer, mint_authority, &mint.pubkey(), &token_account, 1000);
        assert!(result.is_err(), "mint should fail when paused");
    }

    #[test]
    fn test_mint_exceeds_supply_cap_fails() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize with small supply cap
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(100), 6);

        // Create token account
        let user = Keypair::new();
        svm.airdrop(&user.pubkey(), LAMPORTS_PER_SOL).unwrap();
        let token_account = create_token_account(svm, payer, &mint.pubkey());

        // Try to mint more than supply cap - should fail
        let result = program_mint(svm, payer, mint_authority, &mint.pubkey(), &token_account, 200);
        assert!(result.is_err(), "mint should fail when exceeding supply cap");
    }

    #[test]
    fn test_burn_succeeds_when_caller_is_minter() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        // Create token account for mint_authority (who is the minter)
        let token_account = create_token_account_for_owner(svm, payer, mint_authority, &mint.pubkey());
        // mint_tokens(svm, payer, &mint.pubkey(), mint_authority, &token_account, 1000);
        program_mint(svm, payer, mint_authority, &mint.pubkey(), &token_account, 1000).unwrap();

        // Burn tokens using program instruction (mint_authority is minter and owns the ATA)
        let result = program_burn(svm, payer, mint_authority, &mint.pubkey(), &token_account, 500);
        assert!(result.is_ok(), "burn should succeed when called by minter");
    }

    #[test]
    fn test_burn_fails_if_caller_is_not_minter() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        // Create token account for mint_authority and mint tokens
        let token_account = create_token_account_for_owner(svm, payer, mint_authority, &mint.pubkey());
        // mint_tokens(svm, payer, &mint.pubkey(), mint_authority, &token_account, 1000);
        program_mint(svm, payer, mint_authority, &mint.pubkey(), &token_account, 1000).unwrap();

        // Try to burn with non-minter - should fail
        let non_minter = Keypair::new();
        svm.airdrop(&non_minter.pubkey(), LAMPORTS_PER_SOL).unwrap();

        let result = program_burn(svm, payer, &non_minter, &mint.pubkey(), &token_account, 500);
        assert!(result.is_err(), "burn should fail when called by non-minter");
    }

    #[test]
    fn test_burn_more_than_balance() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        // Create token account for mint_authority and mint only 500 tokens
        let token_account = create_token_account_for_owner(svm, payer, mint_authority, &mint.pubkey());
        // mint_tokens(svm, payer, &mint.pubkey(), mint_authority, &token_account, 500);
        program_mint(svm, payer, mint_authority, &mint.pubkey(), &token_account, 500).unwrap();

        // Try to burn more than balance - should fail
        let result = program_burn(svm, payer, mint_authority, &mint.pubkey(), &token_account, 1000);
        assert!(result.is_err(), "burn should fail when burning more than balance");
    }

    #[test]
    fn test_thaw_account_succeeds_when_caller_is_freezer() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize with preset 1 (compliant mode needed for freeze)
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 1, Some(1_000_000_000_000), 6);

        // Create token account and mint tokens
        let token_account = create_token_account_for_owner(svm, payer, mint_authority, &mint.pubkey());
        // mint_tokens(svm, payer, &mint.pubkey(), mint_authority, &token_account, 1000);
        program_mint(svm, payer, mint_authority, &mint.pubkey(), &token_account, 1000).unwrap();

        // Freeze account first
        freeze_account(svm, payer, mint_authority, &mint.pubkey(), &token_account).unwrap();

        // Thaw account - should succeed
        let result = thaw_account(svm, payer, mint_authority, &mint.pubkey(), &token_account);
        assert!(result.is_ok(), "thaw_account should succeed when called by freezer");
    }

    #[test]
    fn test_thaw_account_on_non_frozen_account() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize with preset 1 (compliant mode needed for freeze)
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 1, Some(1_000_000_000_000), 6);

        // Create token account and mint tokens
        let token_account = create_token_account_for_owner(svm, payer, mint_authority, &mint.pubkey());
        // mint_tokens(svm, payer, &mint.pubkey(), mint_authority, &token_account, 1000);
        program_mint(svm, payer, mint_authority, &mint.pubkey(), &token_account, 1000).unwrap();

        // Thaw account without freezing first - SPL Token fails on non-frozen account
        let result = thaw_account(svm, payer, mint_authority, &mint.pubkey(), &token_account);
        // Note: SPL Token returns error for thawing non-frozen account
        assert!(result.is_err(), "thaw_account fails on non-frozen account in SPL Token");
    }

    fn seize(
        svm: &mut LiteSVM,
        payer: &Keypair,
        authority: &Keypair,
        mint: &Pubkey,
        source: &Pubkey,
        destination: &Pubkey,
        source_owner: &Pubkey,
        amount: u64,
    ) -> Result<(), String> {
        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint.as_ref()], &PROGRAM_ID);
        let (source_blacklist, _) =
            Pubkey::find_program_address(&[b"blacklist", config_pda.as_ref(), source_owner.as_ref()], &PROGRAM_ID);
        let token_program = TOKEN_2022_ID;

        let discriminator = compute_instruction_discriminator("seize");
        let args = amount.try_to_vec().unwrap();
        let mut data = serialize_with_discriminator(&discriminator, &args);

        let accounts = vec![
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new(*source, false),
            AccountMeta::new(*destination, false),
            AccountMeta::new_readonly(source_blacklist, false),
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new_readonly(token_program, false),
        ];

        let blockhash = svm.latest_blockhash();

        let tx = Transaction::new_signed_with_payer(
            &[Instruction { program_id: PROGRAM_ID, accounts, data }],
            Some(&payer.pubkey()),
            &[payer, authority],
            blockhash,
        );

        match svm.send_transaction(tx) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    #[test]
    fn test_seize_succeeds_when_caller_is_minter() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize with preset 1 (SSS-2 / compliant mode)
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 1, Some(1_000_000_000_000), 6);

        // Create token accounts for victim and destination
        let victim = Keypair::new();
        let destination = Keypair::new();
        svm.airdrop(&victim.pubkey(), LAMPORTS_PER_SOL).unwrap();
        svm.airdrop(&destination.pubkey(), LAMPORTS_PER_SOL).unwrap();

        let victim_token_account = create_token_account_for_owner(svm, payer, &victim, &mint.pubkey());
        let destination_token_account = create_token_account_for_owner(svm, payer, &destination, &mint.pubkey());

        // Mint tokens to victim
        // mint_tokens(svm, payer, &mint.pubkey(), mint_authority, &victim_token_account, 1000);
        program_mint(svm, payer, mint_authority, &mint.pubkey(), &victim_token_account, 1000).unwrap();

        // Add victim to blacklist
        blacklist_add(svm, payer, mint_authority, &mint.pubkey(), &victim.pubkey(), "Test reason".to_string()).unwrap();

        // Seize tokens - minter has permanent_delegate extension so can transfer from any account
        let result = seize(
            svm,
            payer,
            mint_authority,
            &mint.pubkey(),
            &victim_token_account,
            &destination_token_account,
            &victim.pubkey(),
            500,
        );
        assert!(result.is_ok(), "seize should succeed when called by minter (permanent delegate)");
    }

    #[test]
    fn test_seize_fails_if_caller_is_not_minter() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize with preset 1 (SSS-2 / compliant mode)
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 1, Some(1_000_000_000_000), 6);

        // Create token accounts for victim and destination
        let victim = Keypair::new();
        let destination = Keypair::new();
        let non_seizer = Keypair::new();
        svm.airdrop(&victim.pubkey(), LAMPORTS_PER_SOL).unwrap();
        svm.airdrop(&destination.pubkey(), LAMPORTS_PER_SOL).unwrap();
        svm.airdrop(&non_seizer.pubkey(), LAMPORTS_PER_SOL).unwrap();

        let victim_token_account = create_token_account_for_owner(svm, payer, &victim, &mint.pubkey());
        let destination_token_account = create_token_account_for_owner(svm, payer, &destination, &mint.pubkey());

        // Mint tokens to victim
        // mint_tokens(svm, payer, &mint.pubkey(), mint_authority, &victim_token_account, 1000);
        program_mint(svm, payer, mint_authority, &mint.pubkey(), &victim_token_account, 1000).unwrap();

        // Add victim to blacklist
        blacklist_add(svm, payer, mint_authority, &mint.pubkey(), &victim.pubkey(), "Test reason".to_string()).unwrap();

        // Try to seize with non-minter - should fail
        let result = seize(
            svm,
            payer,
            &non_seizer,
            &mint.pubkey(),
            &victim_token_account,
            &destination_token_account,
            &victim.pubkey(),
            500,
        );
        assert!(result.is_err(), "seize should fail when called by non-minter");
    }

    #[test]
    fn test_seize_fails_if_source_not_blacklisted() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize with preset 1 (SSS-2 / compliant mode)
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 1, Some(1_000_000_000_000), 6);

        // Create token accounts for victim and destination
        let victim = Keypair::new();
        let destination = Keypair::new();
        svm.airdrop(&victim.pubkey(), LAMPORTS_PER_SOL).unwrap();
        svm.airdrop(&destination.pubkey(), LAMPORTS_PER_SOL).unwrap();

        let victim_token_account = create_token_account_for_owner(svm, payer, &victim, &mint.pubkey());
        let destination_token_account = create_token_account_for_owner(svm, payer, &destination, &mint.pubkey());

        // Mint tokens to victim (but DON'T blacklist)
        // mint_tokens(svm, payer, &mint.pubkey(), mint_authority, &victim_token_account, 1000);
        program_mint(svm, payer, mint_authority, &mint.pubkey(), &victim_token_account, 1000).unwrap();

        // Try to seize - should fail (not blacklisted)
        let result = seize(
            svm,
            payer,
            mint_authority,
            &mint.pubkey(),
            &victim_token_account,
            &destination_token_account,
            &victim.pubkey(),
            500,
        );
        assert!(result.is_err(), "seize should fail if source not blacklisted");
    }

    #[test]
    fn test_seize_fails_in_sss1() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize with preset 0 (SSS-1 - no seize)
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        // Create token accounts
        let victim = Keypair::new();
        let destination = Keypair::new();
        svm.airdrop(&victim.pubkey(), LAMPORTS_PER_SOL).unwrap();
        svm.airdrop(&destination.pubkey(), LAMPORTS_PER_SOL).unwrap();

        let victim_token_account = create_token_account_for_owner(svm, payer, &victim, &mint.pubkey());
        let destination_token_account = create_token_account_for_owner(svm, payer, &destination, &mint.pubkey());

        // Mint tokens to victim
        // mint_tokens(svm, payer, &mint.pubkey(), mint_authority, &victim_token_account, 1000);
        program_mint(svm, payer, mint_authority, &mint.pubkey(), &victim_token_account, 1000).unwrap();

        // Try to seize in SSS-1 - should fail
        let result = seize(
            svm,
            payer,
            mint_authority,
            &mint.pubkey(),
            &victim_token_account,
            &destination_token_account,
            &victim.pubkey(),
            500,
        );
        assert!(result.is_err(), "seize should fail in SSS-1");
    }

    #[test]
    fn test_blacklist_add_succeeds_when_caller_is_blacklister() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize with preset 1 (SSS-2 / compliant mode)
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 1, Some(1_000_000_000_000), 6);

        // Add to blacklist - should succeed (master_authority is blacklister by default)
        let target = Keypair::new();
        let result =
            blacklist_add(svm, payer, mint_authority, &mint.pubkey(), &target.pubkey(), "Test reason".to_string());
        assert!(result.is_ok(), "blacklist_add should succeed when called by blacklister");
    }

    #[test]
    fn test_blacklist_add_fails_if_already_blacklisted() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize with preset 1 (SSS-2)
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 1, Some(1_000_000_000_000), 6);

        // Add to blacklist - should succeed first time
        let target = Keypair::new();
        let result =
            blacklist_add(svm, payer, mint_authority, &mint.pubkey(), &target.pubkey(), "Test reason".to_string());
        assert!(result.is_ok(), "blacklist_add should succeed first time");

        // Try to add again - should fail
        let result =
            blacklist_add(svm, payer, mint_authority, &mint.pubkey(), &target.pubkey(), "Test reason".to_string());
        assert!(result.is_err(), "blacklist_add should fail if already blacklisted");
    }

    fn transfer(
        svm: &mut LiteSVM,
        payer: &Keypair,
        authority: &Keypair,
        mint: &Pubkey,
        from: &Pubkey,
        to: &Pubkey,
        from_owner: &Pubkey,
        to_owner: &Pubkey,
        amount: u64,
    ) -> Result<(), String> {
        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint.as_ref()], &PROGRAM_ID);
        let (sender_blacklist, _) =
            Pubkey::find_program_address(&[b"blacklist", config_pda.as_ref(), from_owner.as_ref()], &PROGRAM_ID);
        let (receiver_blacklist, _) =
            Pubkey::find_program_address(&[b"blacklist", config_pda.as_ref(), to_owner.as_ref()], &PROGRAM_ID);
        let token_program = TOKEN_2022_ID;

        let discriminator = compute_instruction_discriminator("transfer");
        let args = amount.try_to_vec().unwrap();
        let mut data = serialize_with_discriminator(&discriminator, &args);

        let accounts = vec![
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(sender_blacklist, false),
            AccountMeta::new_readonly(receiver_blacklist, false),
            AccountMeta::new(*from, false),
            AccountMeta::new(*to, false),
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new_readonly(token_program, false),
        ];

        let blockhash = svm.latest_blockhash();

        let tx = Transaction::new_signed_with_payer(
            &[Instruction { program_id: PROGRAM_ID, accounts, data }],
            Some(&payer.pubkey()),
            &[payer, authority],
            blockhash,
        );

        match svm.send_transaction(tx) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    fn update_transfer_hook(
        svm: &mut LiteSVM,
        payer: &Keypair,
        authority: &Keypair,
        mint: &Pubkey,
        new_hook_program: Option<Pubkey>,
    ) -> Result<(), String> {
        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint.as_ref()], &PROGRAM_ID);

        let discriminator = compute_instruction_discriminator("update_transfer_hook");
        let args = new_hook_program.try_to_vec().unwrap();
        let mut data = serialize_with_discriminator(&discriminator, &args);

        let accounts = vec![AccountMeta::new(config_pda, false), AccountMeta::new(authority.pubkey(), true)];

        let blockhash = svm.latest_blockhash();

        let tx = Transaction::new_signed_with_payer(
            &[Instruction { program_id: PROGRAM_ID, accounts, data }],
            Some(&payer.pubkey()),
            &[payer, authority],
            blockhash,
        );

        match svm.send_transaction(tx) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    fn get_config(svm: &LiteSVM, mint: &Pubkey) -> solana_stablecoin_standard::state::StablecoinConfig {
        let (config_pda, _) = Pubkey::find_program_address(&[b"stablecoin", mint.as_ref()], &PROGRAM_ID);
        let account = svm.get_account(&config_pda).unwrap();
        solana_stablecoin_standard::state::StablecoinConfig::try_deserialize(&mut account.data.as_ref()).unwrap()
    }

    #[test]
    fn test_transfer_succeeds_when_hook_is_none_sss1() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize with preset 0 (SSS-1)
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        // Create token accounts
        let user1 = Keypair::new();
        let user2 = Keypair::new();
        svm.airdrop(&user1.pubkey(), LAMPORTS_PER_SOL).unwrap();
        svm.airdrop(&user2.pubkey(), LAMPORTS_PER_SOL).unwrap();

        let token_account1 = create_token_account_for_owner(svm, payer, &user1, &mint.pubkey());
        let token_account2 = create_token_account_for_owner(svm, payer, &user2, &mint.pubkey());

        // Mint tokens to user1
        // mint_tokens(svm, payer, &mint.pubkey(), mint_authority, &token_account1, 1000);
        program_mint(svm, payer, mint_authority, &mint.pubkey(), &token_account1, 1000).unwrap();

        // Transfer - should succeed (SSS-1 with no hook required)
        let result = transfer(
            svm,
            payer,
            &user1,
            &mint.pubkey(),
            &token_account1,
            &token_account2,
            &user1.pubkey(),
            &user2.pubkey(),
            500,
        );
        assert!(result.is_ok(), "transfer should succeed when hook is None (SSS-1)");
    }

    #[test]
    fn test_transfer_fails_without_hook_in_sss2() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize with preset 1 (SSS-2) but DON'T set hook
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 1, Some(1_000_000_000_000), 6);

        // Create token accounts
        let user1 = Keypair::new();
        let user2 = Keypair::new();
        svm.airdrop(&user1.pubkey(), LAMPORTS_PER_SOL).unwrap();
        svm.airdrop(&user2.pubkey(), LAMPORTS_PER_SOL).unwrap();

        let token_account1 = create_token_account_for_owner(svm, payer, &user1, &mint.pubkey());
        let token_account2 = create_token_account_for_owner(svm, payer, &user2, &mint.pubkey());

        // Mint tokens to user1
        // mint_tokens(svm, payer, &mint.pubkey(), mint_authority, &token_account1, 1000);
        program_mint(svm, payer, mint_authority, &mint.pubkey(), &token_account1, 1000).unwrap();

        // Transfer - should fail (hook not set in SSS-2)
        let result = transfer(
            svm,
            payer,
            &user1,
            &mint.pubkey(),
            &token_account1,
            &token_account2,
            &user1.pubkey(),
            &user2.pubkey(),
            500,
        );
        assert!(result.is_err(), "transfer should fail when hook not set in SSS-2");
    }

    #[test]
    fn test_transfer_succeeds_when_sender_receiver_not_blacklisted() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize with preset 1 (SSS-2)
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 1, Some(1_000_000_000_000), 6);

        // Set transfer hook for compliant mode
        update_transfer_hook(svm, payer, mint_authority, &mint.pubkey(), Some(SSS_PROGRAM_ID)).unwrap();

        // Create token accounts
        let user1 = Keypair::new();
        let user2 = Keypair::new();
        svm.airdrop(&user1.pubkey(), LAMPORTS_PER_SOL).unwrap();
        svm.airdrop(&user2.pubkey(), LAMPORTS_PER_SOL).unwrap();

        let token_account1 = create_token_account_for_owner(svm, payer, &user1, &mint.pubkey());
        let token_account2 = create_token_account_for_owner(svm, payer, &user2, &mint.pubkey());

        // Mint tokens to user1
        // mint_tokens(svm, payer, &mint.pubkey(), mint_authority, &token_account1, 1000);
        program_mint(svm, payer, mint_authority, &mint.pubkey(), &token_account1, 1000).unwrap();

        // Transfer - should succeed (not blacklisted)
        let result = transfer(
            svm,
            payer,
            &user1,
            &mint.pubkey(),
            &token_account1,
            &token_account2,
            &user1.pubkey(),
            &user2.pubkey(),
            500,
        );
        assert!(result.is_ok(), "transfer should succeed when sender/receiver not blacklisted");
    }

    #[test]
    fn test_transfer_fails_when_sender_blacklisted() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize with preset 1 (SSS-2)
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 1, Some(1_000_000_000_000), 6);

        // Set transfer hook for compliant mode
        update_transfer_hook(svm, payer, mint_authority, &mint.pubkey(), Some(SSS_PROGRAM_ID)).unwrap();

        // Create token accounts
        let user1 = Keypair::new();
        let user2 = Keypair::new();
        svm.airdrop(&user1.pubkey(), LAMPORTS_PER_SOL).unwrap();
        svm.airdrop(&user2.pubkey(), LAMPORTS_PER_SOL).unwrap();

        let token_account1 = create_token_account_for_owner(svm, payer, &user1, &mint.pubkey());
        let token_account2 = create_token_account_for_owner(svm, payer, &user2, &mint.pubkey());

        // Mint tokens to user1
        // mint_tokens(svm, payer, &mint.pubkey(), mint_authority, &token_account1, 1000);
        program_mint(svm, payer, mint_authority, &mint.pubkey(), &token_account1, 1000).unwrap();

        // Add user1 to blacklist
        blacklist_add(svm, payer, mint_authority, &mint.pubkey(), &user1.pubkey(), "Test reason".to_string()).unwrap();

        // Transfer - should fail (sender blacklisted)
        let result = transfer(
            svm,
            payer,
            &user1,
            &mint.pubkey(),
            &token_account1,
            &token_account2,
            &user1.pubkey(),
            &user2.pubkey(),
            500,
        );
        assert!(result.is_err(), "transfer should fail when sender is blacklisted");
    }

    #[test]
    fn test_transfer_fails_when_receiver_blacklisted() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize with preset 1 (SSS-2)
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 1, Some(1_000_000_000_000), 6);

        // Set transfer hook for compliant mode
        update_transfer_hook(svm, payer, mint_authority, &mint.pubkey(), Some(SSS_PROGRAM_ID)).unwrap();

        // Create token accounts
        let user1 = Keypair::new();
        let user2 = Keypair::new();
        svm.airdrop(&user1.pubkey(), LAMPORTS_PER_SOL).unwrap();
        svm.airdrop(&user2.pubkey(), LAMPORTS_PER_SOL).unwrap();

        let token_account1 = create_token_account_for_owner(svm, payer, &user1, &mint.pubkey());
        let token_account2 = create_token_account_for_owner(svm, payer, &user2, &mint.pubkey());

        // Mint tokens to user1
        // mint_tokens(svm, payer, &mint.pubkey(), mint_authority, &token_account1, 1000);
        program_mint(svm, payer, mint_authority, &mint.pubkey(), &token_account1, 1000).unwrap();

        // Add user2 to blacklist
        blacklist_add(svm, payer, mint_authority, &mint.pubkey(), &user2.pubkey(), "Test reason".to_string()).unwrap();

        // Transfer - should fail (receiver blacklisted)
        let result = transfer(
            svm,
            payer,
            &user1,
            &mint.pubkey(),
            &token_account1,
            &token_account2,
            &user1.pubkey(),
            &user2.pubkey(),
            500,
        );
        assert!(result.is_err(), "transfer should fail when receiver is blacklisted");
    }

    #[test]
    fn test_transfer_fails_when_paused() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize with preset 0 (SSS-1)
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        // Create token accounts
        let user1 = Keypair::new();
        let user2 = Keypair::new();
        svm.airdrop(&user1.pubkey(), LAMPORTS_PER_SOL).unwrap();
        svm.airdrop(&user2.pubkey(), LAMPORTS_PER_SOL).unwrap();

        let token_account1 = create_token_account_for_owner(svm, payer, &user1, &mint.pubkey());
        let token_account2 = create_token_account_for_owner(svm, payer, &user2, &mint.pubkey());

        // Mint tokens to user1
        // mint_tokens(svm, payer, &mint.pubkey(), mint_authority, &token_account1, 1000);
        program_mint(svm, payer, mint_authority, &mint.pubkey(), &token_account1, 1000).unwrap();

        // Pause the token
        update_paused(svm, payer, mint_authority, &mint.pubkey(), true).unwrap();

        // Transfer - should fail (paused)
        let result = transfer(
            svm,
            payer,
            &user1,
            &mint.pubkey(),
            &token_account1,
            &token_account2,
            &user1.pubkey(),
            &user2.pubkey(),
            500,
        );
        assert!(result.is_err(), "transfer should fail when paused");
    }

    #[test]
    fn test_update_transfer_hook_succeeds_sss2() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize with preset 1 (SSS-2)
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 1, Some(1_000_000_000_000), 6);

        // Update transfer hook - should succeed
        let new_hook = Keypair::new();
        let result = update_transfer_hook(svm, payer, mint_authority, &mint.pubkey(), Some(new_hook.pubkey()));
        assert!(result.is_ok(), "update_transfer_hook should succeed in SSS-2");
    }

    #[test]
    fn test_update_transfer_hook_fails_sss1() {
        let mut setup = setup();
        let svm = &mut setup.svm;
        let payer = &setup.payer;
        let mint = &setup.mint;
        let mint_authority = &setup.mint_authority;

        create_mint(svm, payer, mint, &mint_authority.pubkey(), 6);

        // Initialize with preset 0 (SSS-1)
        let _config = initialize(svm, payer, mint_authority, &mint.pubkey(), 0, Some(1_000_000_000_000), 6);

        // Try to update transfer hook in SSS-1 - should fail
        let new_hook = Keypair::new();
        let result = update_transfer_hook(svm, payer, mint_authority, &mint.pubkey(), Some(new_hook.pubkey()));
        assert!(result.is_err(), "update_transfer_hook should fail in SSS-1");
    }
}
