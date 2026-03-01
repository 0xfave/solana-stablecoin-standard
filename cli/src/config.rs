use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CliConfig {
    pub rpc: Option<String>,
    pub keypair: Option<String>,
    pub private_key: Option<String>,
    pub mint: Option<String>,
    pub program_id: Option<String>,
}

impl CliConfig {
    pub fn load() -> Result<Self, anyhow::Error> {
        let mut paths = vec![std::env::current_dir()?.join("config.toml")];

        if let Some(config_dir) = dirs::config_dir() {
            paths.push(config_dir.join("config.toml"));
        }

        for path in &paths {
            if path.exists() {
                let content = std::fs::read_to_string(path)?;
                return Ok(toml::from_str(&content)?);
            }
        }

        Ok(CliConfig::default())
    }

    pub fn get_rpc(&self) -> String {
        self.rpc.clone().unwrap_or_else(|| "https://api.devnet.solana.com".to_string())
    }

    pub fn get_mint(&self) -> Option<String> {
        self.mint.clone()
    }

    pub fn get_keypair(&self) -> Option<String> {
        self.keypair.clone()
    }

    pub fn get_program_id(&self) -> String {
        self.program_id.clone().unwrap_or_else(|| "Ak5zCGByVQ972WfccBAxR67zZambk5KqUvfEfksUMXr6".to_string())
    }

    pub fn save_mint(&self, mint: &str) -> Result<(), anyhow::Error> {
        let path = std::env::current_dir()?.join("config.toml");

        let mut config = if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            toml::from_str::<CliConfig>(&content).unwrap_or_default()
        } else {
            CliConfig::default()
        };

        config.mint = Some(mint.to_string());

        let content = toml::to_string_pretty(&config)?;
        std::fs::write(&path, content)?;

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StablecoinConfig {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub preset: String,
    pub supply_cap: Option<u64>,
    pub authority: Option<String>,
    pub minters: Option<Vec<String>>,
    pub blacklisted: Option<Vec<BlacklistEntry>>,
    pub paused: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlacklistEntry {
    pub address: String,
    pub reason: String,
}

impl Default for StablecoinConfig {
    fn default() -> Self {
        Self {
            name: "Stablecoin".to_string(),
            symbol: "STB".to_string(),
            decimals: 6,
            preset: "sss-1".to_string(),
            supply_cap: None,
            authority: None,
            minters: None,
            blacklisted: None,
            paused: Some(false),
        }
    }
}

impl StablecoinConfig {
    pub fn from_file(path: &str) -> Result<Self, anyhow::Error> {
        let content = std::fs::read_to_string(path)?;

        if path.ends_with(".toml") {
            Ok(toml::from_str(&content)?)
        } else {
            Ok(serde_json::from_str(&content)?)
        }
    }

    pub fn from_preset(preset: &str) -> Self {
        match preset.to_lowercase().as_str() {
            "sss-1" | "sss_1" | "1" => StablecoinConfig {
                name: "SSS-1 Stablecoin".to_string(),
                symbol: "SSS1".to_string(),
                decimals: 6,
                preset: "sss-1".to_string(),
                supply_cap: Some(1_000_000_000_000),
                authority: None,
                minters: None,
                blacklisted: None,
                paused: Some(false),
            },
            "sss-2" | "sss_2" | "2" => StablecoinConfig {
                name: "SSS-2 Compliant Stablecoin".to_string(),
                symbol: "SSS2".to_string(),
                decimals: 6,
                preset: "sss-2".to_string(),
                supply_cap: Some(1_000_000_000_000),
                authority: None,
                minters: None,
                blacklisted: Some(Vec::new()),
                paused: Some(false),
            },
            _ => StablecoinConfig::default(),
        }
    }
}
