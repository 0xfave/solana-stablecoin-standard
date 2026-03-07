use anyhow::Result;
use solana_sdk::signature::Keypair;
use std::fs;
use std::path::PathBuf;

pub fn load_keypair(path: Option<&str>) -> Result<Keypair> {
    if let Some(keypair_path) = path {
        load_keypair_from_path(&PathBuf::from(keypair_path))
    } else if let Ok(keypair_str) = std::env::var("SOLANA_KEYPAIR") {
        load_keypair_from_path(&PathBuf::from(keypair_str))
    } else {
        let default_path = dirs::home_dir()
            .map(|p| p.join(".config").join("solana").join("id.json"))
            .ok_or_else(|| anyhow::anyhow!("Cannot find default keypair path"))?;

        if default_path.exists() {
            load_keypair_from_path(&default_path)
        } else {
            Err(anyhow::anyhow!(
                "No keypair found. Provide --keypair, or use default location ~/.config/solana/id.json"
            ))
        }
    }
}

pub fn load_keypair_from_path(path: &PathBuf) -> Result<Keypair> {
    let content = fs::read_to_string(path)?;

    let bytes: Vec<u8> =
        if content.trim().starts_with('[') { serde_json::from_str(&content)? } else { content.as_bytes().to_vec() };

    if let Ok(keypair) = Keypair::try_from(bytes.as_slice()) {
        return Ok(keypair);
    }
    Err(anyhow::anyhow!("Invalid keypair file"))
}

pub fn get_program_id() -> String {
    std::env::var("SSS_PROGRAM_ID").unwrap_or_else(|_| "C78Fk7ZeyGuQV92u3aKJQSeXMn35A9Jrjeyv33UNE4Nw".to_string())
}

pub fn get_token_2022_program_id() -> String {
    "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".to_string()
}

pub fn get_system_program_id() -> String {
    "11111111111111111111111111111111".to_string()
}
