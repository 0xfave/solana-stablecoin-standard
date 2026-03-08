use serde::{Deserialize, Serialize};
use solana_client::rpc_client::RpcClient as SolanaRpcClient;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use tracing::{error, info};

// ✅ Correct program ID
pub const PROGRAM_ID: &str = "C78Fk7ZeyGuQV92u3aKJQSeXMn35A9Jrjeyv33UNE4Nw";
pub const TOKEN_2022_PROGRAM_ID: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
pub const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

fn derive_config_pda(mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"stablecoin", &mint.to_bytes()], &PROGRAM_ID.parse::<Pubkey>().unwrap())
}

/// Derives ATA using the correct 3-seed pattern for Token-2022.
fn derive_ata(wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
    let token_program: Pubkey = TOKEN_2022_PROGRAM_ID.parse().unwrap();
    let assoc_program: Pubkey = ASSOCIATED_TOKEN_PROGRAM_ID.parse().unwrap();
    let (ata, _) =
        Pubkey::find_program_address(&[wallet.as_ref(), token_program.as_ref(), mint.as_ref()], &assoc_program);
    ata
}

fn discriminator(name: &str) -> [u8; 8] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{}", name));
    let result = hasher.finalize();
    result[..8].try_into().unwrap()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResult {
    pub signature: String,
    pub success: bool,
    pub error: Option<String>,
}

pub struct SolanaService {
    rpc: SolanaRpcClient,
    payer: Keypair,
    program_id: Pubkey,
    token_program_id: Pubkey,
}

impl SolanaService {
    pub fn new(rpc_url: &str, private_key: &str) -> Result<Self, String> {
        // Accept base58-encoded keypair bytes
        let decoded =
            bs58::decode(private_key).into_vec().map_err(|e| format!("Failed to decode private key: {}", e))?;
        let payer = Keypair::try_from(decoded.as_slice()).map_err(|e| format!("Failed to parse keypair: {}", e))?;
        let rpc = SolanaRpcClient::new(rpc_url);
        let program_id: Pubkey = PROGRAM_ID.parse().map_err(|_| "Invalid program ID")?;
        let token_program_id: Pubkey = TOKEN_2022_PROGRAM_ID.parse().map_err(|_| "Invalid token program ID")?;
        Ok(Self { rpc, payer, program_id, token_program_id })
    }

    pub fn payer_pubkey(&self) -> Pubkey {
        self.payer.pubkey()
    }

    /// Initialize a new stablecoin mint.
    /// Program instruction: initialize(supply_cap: Option<u64>, decimals: u8)
    /// No preset arg — mint type is determined by which modules are attached afterward.
    pub fn initialize(&self, supply_cap: Option<u64>, decimals: u8) -> Result<(Pubkey, Pubkey), String> {
        let mint_keypair = Keypair::new();
        let mint = mint_keypair.pubkey();
        let (config, _) = derive_config_pda(&mint);

        // All SSS mints need PermanentDelegate so the config PDA can seize tokens.
        let mint_space = spl_token_2022::extension::ExtensionType::try_calculate_account_len::<
            spl_token_2022::state::Mint,
        >(&[spl_token_2022::extension::ExtensionType::PermanentDelegate])
        .map_err(|e| format!("Failed to calculate mint space: {}", e))?;

        let lamports = self
            .rpc
            .get_minimum_balance_for_rent_exemption(mint_space)
            .map_err(|e| format!("Failed to get rent: {}", e))?;

        let create_account_ix = solana_sdk::system_instruction::create_account(
            &self.payer.pubkey(),
            &mint,
            lamports,
            mint_space as u64,
            &self.token_program_id,
        );

        // PermanentDelegate must be initialized BEFORE initialize_mint2
        let init_delegate_ix = spl_token_2022::instruction::initialize_permanent_delegate(
            &self.token_program_id,
            &mint,
            &config, // config PDA is the permanent delegate
        )
        .map_err(|e| format!("Failed to create permanent delegate ix: {}", e))?;

        let init_mint_ix = spl_token_2022::instruction::initialize_mint2(
            &self.token_program_id,
            &mint,
            &self.payer.pubkey(),
            Some(&self.payer.pubkey()),
            decimals,
        )
        .map_err(|e| format!("Failed to create init mint ix: {}", e))?;

        // ✅ Correct initialize discriminator and args: (supply_cap: Option<u64>, decimals: u8)
        let mut data = discriminator("initialize").to_vec();
        match supply_cap {
            Some(cap) => {
                data.push(1); // Some
                data.extend_from_slice(&cap.to_le_bytes());
            }
            None => {
                data.push(0); // None
            }
        }
        data.push(decimals);

        let init_program_ix = Instruction::new_with_bytes(
            self.program_id,
            &data,
            vec![
                AccountMeta::new(config, false),
                AccountMeta::new(mint, true),
                AccountMeta::new(self.payer.pubkey(), true),
                AccountMeta::new_readonly(self.token_program_id, false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            ],
        );

        let recent_blockhash =
            self.rpc.get_latest_blockhash().map_err(|e| format!("Failed to get blockhash: {}", e))?;

        // ✅ mut tx so we can sign it
        let mut tx = Transaction::new_with_payer(
            &[create_account_ix, init_delegate_ix, init_mint_ix, init_program_ix],
            Some(&self.payer.pubkey()),
        );
        tx.sign(&[&self.payer, &mint_keypair], recent_blockhash);

        match self.rpc.send_and_confirm_transaction_with_spinner(&tx) {
            Ok(sig) => {
                info!("Initialize successful: {}", sig);
                Ok((mint, config))
            }
            Err(e) => {
                error!("Initialize failed: {}", e);
                Err(format!("Initialize failed: {}", e))
            }
        }
    }

    /// Mint tokens to a recipient wallet. Creates ATA if needed.
    /// ✅ Correct discriminator: "mint_tokens", not "mint"
    pub fn mint_tokens(&self, mint: &Pubkey, recipient: &Pubkey, amount: u64) -> TransactionResult {
        let (config, _) = derive_config_pda(mint);
        let recipient_ata = derive_ata(recipient, mint);

        if let Err(e) = self.create_ata_if_needed(mint, recipient, &recipient_ata) {
            return TransactionResult { signature: String::new(), success: false, error: Some(e) };
        }

        let mut data = discriminator("mint_tokens").to_vec();
        data.extend_from_slice(&amount.to_le_bytes());

        let ix = Instruction::new_with_bytes(
            self.program_id,
            &data,
            vec![
                AccountMeta::new_readonly(config, false),             // config
                AccountMeta::new(*mint, false),                       // mint (writable)
                AccountMeta::new(recipient_ata, false),               // destination (writable)
                AccountMeta::new_readonly(self.payer.pubkey(), true), // minter (signer)
                AccountMeta::new_readonly(self.token_program_id, false),
            ],
        );

        let recent_blockhash = match self.rpc.get_latest_blockhash() {
            Ok(bh) => bh,
            Err(e) => {
                return TransactionResult { signature: String::new(), success: false, error: Some(e.to_string()) }
            }
        };

        let mut tx = Transaction::new_with_payer(&[ix], Some(&self.payer.pubkey()));
        tx.sign(&[&self.payer], recent_blockhash);

        match self.rpc.send_and_confirm_transaction_with_spinner(&tx) {
            Ok(sig) => {
                info!("Mint successful: {}", sig);
                TransactionResult { signature: sig.to_string(), success: true, error: None }
            }
            Err(e) => {
                error!("Mint failed: {}", e);
                TransactionResult { signature: String::new(), success: false, error: Some(e.to_string()) }
            }
        }
    }

    /// Burn tokens from a token account.
    /// ✅ Correct discriminator: "burn_tokens", not "burn"
    pub fn burn_tokens(&self, mint: &Pubkey, from_ata: &Pubkey, amount: u64) -> TransactionResult {
        let (config, _) = derive_config_pda(mint);

        let mut data = discriminator("burn_tokens").to_vec();
        data.extend_from_slice(&amount.to_le_bytes());

        let ix = Instruction::new_with_bytes(
            self.program_id,
            &data,
            vec![
                AccountMeta::new_readonly(config, false),             // config
                AccountMeta::new(*mint, false),                       // mint (writable)
                AccountMeta::new(*from_ata, false),                   // from (writable)
                AccountMeta::new_readonly(self.payer.pubkey(), true), // burner (signer)
                AccountMeta::new_readonly(self.token_program_id, false),
            ],
        );

        let recent_blockhash = match self.rpc.get_latest_blockhash() {
            Ok(bh) => bh,
            Err(e) => {
                return TransactionResult { signature: String::new(), success: false, error: Some(e.to_string()) }
            }
        };

        let mut tx = Transaction::new_with_payer(&[ix], Some(&self.payer.pubkey()));
        tx.sign(&[&self.payer], recent_blockhash);

        match self.rpc.send_and_confirm_transaction_with_spinner(&tx) {
            Ok(sig) => {
                info!("Burn successful: {}", sig);
                TransactionResult { signature: sig.to_string(), success: true, error: None }
            }
            Err(e) => {
                error!("Burn failed: {}", e);
                TransactionResult { signature: String::new(), success: false, error: Some(e.to_string()) }
            }
        }
    }

    fn create_ata_if_needed(&self, mint: &Pubkey, recipient: &Pubkey, ata: &Pubkey) -> Result<(), String> {
        if self.rpc.get_account(ata).is_ok() {
            return Ok(());
        }

        let assoc_program: Pubkey = ASSOCIATED_TOKEN_PROGRAM_ID.parse().unwrap();

        // ATA program expects: [funder, ata, wallet, mint, system_program, token_program]
        // with no instruction data — the program derives the ATA itself.
        let create_ata_ix = Instruction::new_with_bytes(
            assoc_program,
            &[],
            vec![
                AccountMeta::new(self.payer.pubkey(), true),  // funder (signer, writable)
                AccountMeta::new(*ata, false),                // ata (writable)
                AccountMeta::new_readonly(*recipient, false), // wallet
                AccountMeta::new_readonly(*mint, false),      // mint
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                AccountMeta::new_readonly(self.token_program_id, false),
            ],
        );

        let recent_blockhash =
            self.rpc.get_latest_blockhash().map_err(|e| format!("Failed to get blockhash: {}", e))?;

        let mut tx = Transaction::new_with_payer(&[create_ata_ix], Some(&self.payer.pubkey()));
        tx.sign(&[&self.payer], recent_blockhash);

        self.rpc.send_and_confirm_transaction_with_spinner(&tx).map_err(|e| format!("Failed to create ATA: {}", e))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instruction_discriminators() {
        // ✅ Correct names matching the deployed program
        let names = [
            "initialize",
            "mint_tokens",
            "burn_tokens",
            "freeze_account",
            "thaw_account",
            "seize",
            "blacklist_add",
            "blacklist_remove",
            "update_paused",
            "attach_compliance_module",
            "detach_compliance_module",
        ];
        for name in &names {
            let disc = discriminator(name);
            assert_eq!(disc.len(), 8, "discriminator for '{}' should be 8 bytes", name);
        }
    }
}
