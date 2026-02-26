use crate::test_util::*;
use anchor_lang::prelude::Pubkey;
use anchor_lang::AccountDeserialize;
use solana_signer::Signer;
use solana_stablecoin_standard::{instruction as sss_instruction};

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
