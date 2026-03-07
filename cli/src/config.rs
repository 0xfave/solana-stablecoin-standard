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
        self.program_id.clone().unwrap_or_else(|| "C78Fk7ZeyGuQV92u3aKJQSeXMn35A9Jrjeyv33UNE4Nw".to_string())
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

// Describes a token to create. Modules are attached separately after creation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StablecoinConfig {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub supply_cap: Option<u64>,
    pub authority: Option<String>,
    pub minters: Option<Vec<String>>,
    pub paused: Option<bool>,
    // Optional: attach compliance module on creation
    pub compliance: Option<ComplianceConfig>,
    // Optional: attach privacy module on creation
    pub privacy: Option<PrivacyConfig>,
}

// Config for the compliance module (SSS-2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceConfig {
    pub blacklister: String,
}

// Config for the privacy module (SSS-3)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyConfig {
    pub allowlist_authority: String,
    pub confidential_transfers: Option<bool>,
}

impl Default for StablecoinConfig {
    fn default() -> Self {
        Self {
            name: "Stablecoin".to_string(),
            symbol: "STB".to_string(),
            decimals: 6,
            supply_cap: None,
            authority: None,
            minters: None,
            paused: Some(false),
            compliance: None,
            privacy: None,
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

    /// Convenience constructors matching the SSS tier naming.
    /// Modules are optional — attach them after creation or inline here.
    pub fn from_tier(tier: &str) -> Self {
        match tier.to_lowercase().as_str() {
            // SSS-1: basic token, no modules
            "sss-1" | "sss_1" | "1" => StablecoinConfig {
                name: "SSS-1 Stablecoin".to_string(),
                symbol: "SSS1".to_string(),
                decimals: 6,
                supply_cap: Some(1_000_000_000_000),
                compliance: None,
                privacy: None,
                ..Default::default()
            },
            // SSS-2: compliance module attached
            "sss-2" | "sss_2" | "2" => StablecoinConfig {
                name: "SSS-2 Compliant Stablecoin".to_string(),
                symbol: "SSS2".to_string(),
                decimals: 6,
                supply_cap: Some(1_000_000_000_000),
                compliance: Some(ComplianceConfig {
                    blacklister: String::new(), // caller must fill in
                }),
                privacy: None,
                ..Default::default()
            },
            // SSS-3: privacy module attached (compliance optional)
            "sss-3" | "sss_3" | "3" => StablecoinConfig {
                name: "SSS-3 Privacy Stablecoin".to_string(),
                symbol: "SSS3".to_string(),
                decimals: 6,
                supply_cap: Some(1_000_000_000_000),
                compliance: None,
                privacy: Some(PrivacyConfig {
                    allowlist_authority: String::new(), // caller must fill in
                    confidential_transfers: Some(false),
                }),
                ..Default::default()
            },
            _ => StablecoinConfig::default(),
        }
    }

    /// Whether the compliance module should be attached on creation
    pub fn has_compliance(&self) -> bool {
        self.compliance.is_some()
    }

    /// Whether the privacy module should be attached on creation
    pub fn has_privacy(&self) -> bool {
        self.privacy.is_some()
    }
}
