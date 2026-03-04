use serde::{Deserialize, Serialize};
use solana_client::rpc_client::RpcClient as SolanaRpcClient;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use std::str::FromStr;
use tracing::{error, info};

pub const PROGRAM_ID: &str = "Ak5zCGByVQ972WfccBAxR67zZambk5KqUvfEfksUMXr6";
pub const TOKEN_2022_PROGRAM_ID: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";

fn derive_config_pda(mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"stablecoin", &mint.to_bytes()], &PROGRAM_ID.parse().unwrap())
}

fn derive_ata(wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
    let token_program_id: Pubkey = Pubkey::from_str(TOKEN_2022_PROGRAM_ID).unwrap();
    let associated_token_program: Pubkey = Pubkey::from_str("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL").unwrap();
    let (ata, _) = Pubkey::find_program_address(
        &[wallet.as_ref(), token_program_id.as_ref(), mint.as_ref()],
        &associated_token_program,
    );
    ata
}

fn get_instruction_discriminator(name: &str) -> [u8; 8] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{}", name));
    let result = hasher.finalize();
    let mut discriminator = [0u8; 8];
    discriminator.copy_from_slice(&result[..8]);
    discriminator
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

    pub fn mint_tokens(&self, mint: &Pubkey, recipient: &Pubkey, amount: u64) -> TransactionResult {
        let (config, _) = derive_config_pda(mint);
        let recipient_ata = derive_ata(recipient, mint);

        if let Err(e) = self.create_ata_if_needed(mint, recipient, &recipient_ata) {
            return TransactionResult { signature: String::new(), success: false, error: Some(e) };
        }

        let discriminator = get_instruction_discriminator("mint");
        let mut data = discriminator.to_vec();
        data.extend_from_slice(&amount.to_le_bytes());

        let instruction = Instruction::new_with_bytes(
            self.program_id,
            &data,
            vec![
                AccountMeta::new_readonly(config, false),
                AccountMeta::new(*mint, false),
                AccountMeta::new(recipient_ata, false),
                AccountMeta::new_readonly(self.payer.pubkey(), true),
                AccountMeta::new_readonly(self.token_program_id, false),
            ],
        );

        let recent_blockhash = match self.rpc.get_latest_blockhash() {
            Ok(bh) => bh,
            Err(e) => {
                error!("Failed to get blockhash: {}", e);
                return TransactionResult { signature: String::new(), success: false, error: Some(e.to_string()) };
            }
        };

        let mut tx = Transaction::new_with_payer(&[instruction], Some(&self.payer.pubkey()));
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

    pub fn initialize(&self, preset: u8, supply_cap: Option<u64>, decimals: u8) -> Result<(Pubkey, Pubkey), String> {
        let mint = Keypair::new();
        let (config, _) = derive_config_pda(&mint.pubkey());

        // Calculate mint space - larger if preset=1 (needs Permanent Delegate extension)
        let mint_space = if preset == 1 {
            // Include PermanentDelegate extension for SSS-2 (preset=1)
            use spl_token_2022::extension::ExtensionType;
            ExtensionType::try_calculate_account_len::<spl_token_2022::state::Mint>(&[
                ExtensionType::PermanentDelegate,
            ])
            .unwrap()
        } else {
            82 // Basic mint size for preset=0
        };

        let lamports = self
            .rpc
            .get_minimum_balance_for_rent_exemption(mint_space)
            .map_err(|e| format!("Failed to get lamports: {}", e))?;

        let create_mint_ix = system_instruction::create_account(
            &self.payer.pubkey(),
            &mint.pubkey(),
            lamports,
            mint_space as u64,
            &self.token_program_id,
        );

        // For preset=1 (SSS-2), add Permanent Delegate extension
        let mut instructions = vec![create_mint_ix];

        if preset == 1 {
            let init_permanent_delegate_ix = spl_token_2022::instruction::initialize_permanent_delegate(
                &self.token_program_id,
                &mint.pubkey(),
                &config,
            )
            .map_err(|e| format!("Failed to create permanent delegate ix: {}", e))?;
            instructions.push(init_permanent_delegate_ix);
        }

        let init_mint_ix = spl_token_2022::instruction::initialize_mint2(
            &self.token_program_id,
            &mint.pubkey(),
            &self.payer.pubkey(),
            Some(&self.payer.pubkey()),
            decimals,
        )
        .map_err(|e| format!("Failed to create init mint ix: {}", e))?;
        instructions.push(init_mint_ix);

        let init_program_ix = self.create_initialize_instruction(preset, supply_cap, decimals, &mint.pubkey(), &config);
        instructions.push(init_program_ix);

        let recent_blockhash =
            self.rpc.get_latest_blockhash().map_err(|e| format!("Failed to get blockhash: {}", e))?;

        let tx =
            Transaction::new_with_payer(&instructions, Some(&self.payer.pubkey()));
        tx.sign(&[&self.payer, &mint], recent_blockhash);

        match self.rpc.send_and_confirm_transaction_with_spinner(&tx) {
            Ok(sig) => {
                info!("Initialize successful: {}", sig);
                Ok((mint.pubkey(), config))
            }
            Err(e) => {
                error!("Initialize failed: {}", e);
                Err(format!("Initialize failed: {}", e))
            }
        }
    }

    fn create_initialize_instruction(
        &self,
        preset: u8,
        supply_cap: Option<u64>,
        decimals: u8,
        mint: &Pubkey,
        config: &Pubkey,
    ) -> Instruction {
        let discriminator = get_instruction_discriminator("initialize");
        let mut data = discriminator.to_vec();

        data.push(preset);

        if let Some(cap) = supply_cap {
            data.push(1);
            data.extend_from_slice(&cap.to_le_bytes());
        } else {
            data.push(0);
            data.extend_from_slice(&[0u8; 8]);
        }

        data.push(decimals);

        Instruction::new_with_bytes(
            self.program_id,
            &data,
            vec![
                AccountMeta::new(*config, false),
                AccountMeta::new(*mint, true),
                AccountMeta::new(self.payer.pubkey(), true),
                AccountMeta::new_readonly(self.token_program_id, false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            ],
        )
    }

    fn create_ata_if_needed(&self, mint: &Pubkey, recipient: &Pubkey, ata: &Pubkey) -> Result<(), String> {
        let ata_info = self.rpc.get_account(ata).ok();

        if ata_info.is_some() {
            return Ok(());
        }

        let associated_token_program: Pubkey =
            Pubkey::from_str("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL").unwrap();

        let create_ata_ix = Instruction::new_with_bytes(
            associated_token_program,
            &[],
            vec![
                AccountMeta::new_readonly(self.payer.pubkey(), true),
                AccountMeta::new(*ata, false),
                AccountMeta::new_readonly(*recipient, false),
                AccountMeta::new_readonly(*mint, false),
                AccountMeta::new_readonly(self.payer.pubkey(), true),
                AccountMeta::new_readonly(self.token_program_id, false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
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
        let mint_disc = get_instruction_discriminator("mint");
        assert_eq!(mint_disc.len(), 8);

        let burn_disc = get_instruction_discriminator("burn");
        assert_eq!(burn_disc.len(), 8);

        let transfer_disc = get_instruction_discriminator("transfer");
        assert_eq!(transfer_disc.len(), 8);
    }
}
