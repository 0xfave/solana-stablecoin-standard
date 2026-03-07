use std::io;
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, BorderType, Clear, List, ListItem, ListState, Padding,
        Paragraph, Wrap,
    },
    Frame, Terminal,
};
use std::time::{Duration, Instant};

use crate::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use std::str::FromStr;

// ─── Colour palette ──────────────────────────────────────────────────────────

const CYAN: Color = Color::Rgb(37, 209, 244);
const DARK: Color = Color::Rgb(14, 14, 18);
const PANEL: Color = Color::Rgb(20, 20, 23);
const DIM: Color = Color::DarkGray;
const GREEN: Color = Color::Rgb(74, 222, 128);
const ORANGE: Color = Color::Rgb(251, 146, 60);
const RED: Color = Color::Rgb(248, 113, 113);
const PURPLE: Color = Color::Rgb(167, 139, 250);

// ─── App state ───────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
enum Section {
    Init,
    Status,
    MintBurn,
    FreezeThaw,
    Blacklist,
    Minters,
    Modules,
}

impl Section {
    fn title(&self) -> &str {
        match self {
            Section::Init => "Create Token",
            Section::Status => "Status / Supply",
            Section::MintBurn => "Mint / Burn",
            Section::FreezeThaw => "Freeze / Thaw",
            Section::Blacklist => "Blacklist / Seize",
            Section::Minters => "Minters",
            Section::Modules => "Modules",
        }
    }

    fn all() -> Vec<Section> {
        vec![
            Section::Init,
            Section::Status,
            Section::MintBurn,
            Section::FreezeThaw,
            Section::Blacklist,
            Section::Minters,
            Section::Modules,
        ]
    }
}

#[derive(Clone, PartialEq)]
enum InputMode {
    Normal,
    Editing,
}

#[derive(Clone)]
struct InputField {
    label: String,
    value: String,
    placeholder: String,
}

impl InputField {
    fn new(label: &str, placeholder: &str) -> Self {
        Self {
            label: label.to_string(),
            value: String::new(),
            placeholder: placeholder.to_string(),
        }
    }
}

#[derive(Clone, PartialEq)]
enum ActionModal {
    None,
    Init,
    Mint,
    Burn,
    Freeze,
    Thaw,
    BlacklistAdd,
    BlacklistRemove,
    Seize,
    AddMinter,
    RemoveMinter,
    AttachCompliance,
    DetachCompliance,
    AttachPrivacy,
    DetachPrivacy,
    AllowlistAdd,
    AllowlistRemove,
}

struct App {
    menu_state: ListState,
    selected_section: Section,

    mint_address: Option<String>,
    supply: Option<String>,
    paused: bool,
    compliance_attached: bool,
    privacy_attached: bool,
    wallet: String,
    status_lines: Vec<String>,

    input_mode: InputMode,
    fields: Vec<InputField>,
    focused_field: usize,

    modal: ActionModal,

    messages: Vec<(String, Color)>,
    loading: bool,
    loading_text: String,

    blacklist_entries: Vec<String>,
    minter_entries: Vec<String>,
    list_state: ListState,

    last_tick: Instant,
    tick_count: u64,
}

impl App {
    fn new(wallet: String, mint: Option<String>) -> Self {
        let mut menu_state = ListState::default();
        menu_state.select(Some(0));
        let mut list_state = ListState::default();
        list_state.select(Some(0));

        Self {
            menu_state,
            selected_section: Section::Init,
            mint_address: mint,
            supply: None,
            paused: false,
            compliance_attached: false,
            privacy_attached: false,
            wallet,
            status_lines: Vec::new(),
            input_mode: InputMode::Normal,
            fields: Vec::new(),
            focused_field: 0,
            modal: ActionModal::None,
            messages: Vec::new(),
            loading: false,
            loading_text: String::new(),
            blacklist_entries: Vec::new(),
            minter_entries: Vec::new(),
            list_state,
            last_tick: Instant::now(),
            tick_count: 0,
        }
    }

    fn push_msg(&mut self, msg: impl Into<String>, color: Color) {
        self.messages.push((msg.into(), color));
        if self.messages.len() > 6 {
            self.messages.remove(0);
        }
    }

    fn select_menu(&mut self, idx: usize) {
        let sections = Section::all();
        if idx < sections.len() {
            self.menu_state.select(Some(idx));
            self.selected_section = sections[idx].clone();
            self.modal = ActionModal::None;
            self.input_mode = InputMode::Normal;
        }
    }

    fn menu_up(&mut self) {
        let i = self.menu_state.selected().unwrap_or(0);
        let next = if i == 0 { Section::all().len() - 1 } else { i - 1 };
        self.select_menu(next);
    }

    fn menu_down(&mut self) {
        let i = self.menu_state.selected().unwrap_or(0);
        let next = (i + 1) % Section::all().len();
        self.select_menu(next);
    }

    fn tier(&self) -> &str {
        match (self.compliance_attached, self.privacy_attached) {
            (_, true) => "SSS-3",
            (true, false) => "SSS-2",
            _ => "SSS-1",
        }
    }

    fn open_modal(&mut self, modal: ActionModal) {
        self.fields.clear();
        self.focused_field = 0;
        self.input_mode = InputMode::Editing;

        match &modal {
            ActionModal::Init => {
                self.fields.push(InputField::new("Name", "e.g. My USD Coin"));
                self.fields.push(InputField::new("Symbol", "e.g. USDC"));
                self.fields.push(InputField::new("Decimals", "6"));
                self.fields.push(InputField::new("Supply Cap", "e.g. 1000000000000 (leave blank for none)"));
            }
            ActionModal::Mint => {
                self.fields.push(InputField::new("Recipient", "Wallet address"));
                self.fields.push(InputField::new("Amount", "e.g. 1000"));
            }
            ActionModal::Burn => {
                self.fields.push(InputField::new("Amount", "e.g. 500"));
            }
            ActionModal::Freeze | ActionModal::Thaw => {
                self.fields.push(InputField::new("Address", "Wallet address"));
            }
            ActionModal::BlacklistAdd => {
                self.fields.push(InputField::new("Address", "Wallet address to blacklist"));
                self.fields.push(InputField::new("Reason", "e.g. suspicious activity"));
            }
            ActionModal::BlacklistRemove => {
                self.fields.push(InputField::new("Address", "Wallet address to remove"));
            }
            ActionModal::Seize => {
                self.fields.push(InputField::new("From", "Blacklisted wallet address"));
                self.fields.push(InputField::new("To", "Destination wallet address"));
                self.fields.push(InputField::new("Amount", "Amount in smallest units"));
            }
            ActionModal::AddMinter => {
                self.fields.push(InputField::new("Address", "Minter wallet address"));
            }
            ActionModal::RemoveMinter => {
                self.fields.push(InputField::new("Address", "Minter address to remove"));
            }
            ActionModal::AttachCompliance => {
                self.fields.push(InputField::new("Blacklister", "Blacklister wallet address"));
            }
            ActionModal::AttachPrivacy => {
                self.fields.push(InputField::new("Allowlist Authority", "Allowlist authority address"));
            }
            ActionModal::AllowlistAdd => {
                self.fields.push(InputField::new("Address", "Wallet to allowlist"));
            }
            ActionModal::AllowlistRemove => {
                self.fields.push(InputField::new("Address", "Wallet to remove from allowlist"));
            }
            // Confirm-only — no fields
            ActionModal::DetachCompliance | ActionModal::DetachPrivacy => {}
            _ => {}
        }

        self.modal = modal;
    }

    fn current_field_input(&mut self, c: char) {
        if self.focused_field < self.fields.len() {
            self.fields[self.focused_field].value.push(c);
        }
    }

    fn current_field_backspace(&mut self) {
        if self.focused_field < self.fields.len() {
            self.fields[self.focused_field].value.pop();
        }
    }

    fn next_field(&mut self) {
        if self.focused_field + 1 < self.fields.len() {
            self.focused_field += 1;
        }
    }

    fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
    }
}

// ─── Entry point ─────────────────────────────────────────────────────────────

pub async fn run(rpc_client: &RpcClient, keypair: &Keypair, mint: Option<String>) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let wallet = keypair.pubkey().to_string();
    let mut app = App::new(wallet, mint);

    let result = run_loop(&mut terminal, &mut app, rpc_client, keypair).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    result
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    rpc_client: &RpcClient,
    keypair: &Keypair,
) -> Result<()> {
    let tick_rate = Duration::from_millis(100);

    // Initial fetch only if mint is already set
    if app.mint_address.is_some() {
        fetch_status(app, rpc_client, keypair).await;
        app.select_menu(1); // jump straight to Status
    }

    loop {
        terminal.draw(|f| ui(f, app))?;

        let timeout = tick_rate
            .checked_sub(app.last_tick.elapsed())
            .unwrap_or(Duration::ZERO);

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Release {
                    continue;
                }
                if app.modal != ActionModal::None {
                    app.input_mode = InputMode::Editing;
                }
                if key.code == KeyCode::Char('q') && app.input_mode == InputMode::Normal {
                    return Ok(());
                }
                match app.input_mode {
                    InputMode::Normal => handle_normal_key(app, key.code, rpc_client, keypair).await,
                    InputMode::Editing => handle_editing_key(app, key.code, rpc_client, keypair).await,
                }
            }
        }

        if app.last_tick.elapsed() >= tick_rate {
            app.tick();
            app.last_tick = Instant::now();
        }
    }
}

// ─── Key handlers ─────────────────────────────────────────────────────────────

async fn handle_normal_key(app: &mut App, key: KeyCode, rpc_client: &RpcClient, keypair: &Keypair) {
    match key {
        KeyCode::Up | KeyCode::Char('k') => app.menu_up(),
        KeyCode::Down | KeyCode::Char('j') => app.menu_down(),
        KeyCode::Esc => app.modal = ActionModal::None,

        KeyCode::Char('n') if app.selected_section == Section::Init => {
            app.open_modal(ActionModal::Init);
        }
        KeyCode::Char('m') if app.selected_section == Section::MintBurn => {
            app.open_modal(ActionModal::Mint);
        }
        KeyCode::Char('b') if app.selected_section == Section::MintBurn => {
            app.open_modal(ActionModal::Burn);
        }
        KeyCode::Char('f') if app.selected_section == Section::FreezeThaw => {
            app.open_modal(ActionModal::Freeze);
        }
        KeyCode::Char('t') if app.selected_section == Section::FreezeThaw => {
            app.open_modal(ActionModal::Thaw);
        }
        KeyCode::Char('a') if app.selected_section == Section::Blacklist => {
            app.open_modal(ActionModal::BlacklistAdd);
        }
        KeyCode::Char('r') if app.selected_section == Section::Blacklist => {
            app.open_modal(ActionModal::BlacklistRemove);
        }
        KeyCode::Char('s') if app.selected_section == Section::Blacklist => {
            app.open_modal(ActionModal::Seize);
        }
        KeyCode::Char('a') if app.selected_section == Section::Minters => {
            app.open_modal(ActionModal::AddMinter);
        }
        KeyCode::Char('r') if app.selected_section == Section::Minters => {
            app.open_modal(ActionModal::RemoveMinter);
        }
        KeyCode::Char('1') if app.selected_section == Section::Modules => {
            app.open_modal(ActionModal::AttachCompliance);
        }
        KeyCode::Char('2') if app.selected_section == Section::Modules => {
            app.open_modal(ActionModal::DetachCompliance);
        }
        KeyCode::Char('3') if app.selected_section == Section::Modules => {
            app.open_modal(ActionModal::AttachPrivacy);
        }
        KeyCode::Char('4') if app.selected_section == Section::Modules => {
            app.open_modal(ActionModal::DetachPrivacy);
        }
        KeyCode::Char('5') if app.selected_section == Section::Modules => {
            app.open_modal(ActionModal::AllowlistAdd);
        }
        KeyCode::Char('6') if app.selected_section == Section::Modules => {
            app.open_modal(ActionModal::AllowlistRemove);
        }
        KeyCode::Char('r') => {
            fetch_status(app, rpc_client, keypair).await;
        }
        KeyCode::Enter if app.selected_section == Section::Status => {
            fetch_status(app, rpc_client, keypair).await;
        }
        _ => {}
    }
}

async fn handle_editing_key(app: &mut App, key: KeyCode, rpc_client: &RpcClient, keypair: &Keypair) {
    match key {
        KeyCode::Esc => {
            app.modal = ActionModal::None;
            app.input_mode = InputMode::Normal;
            app.fields.clear();
        }
        KeyCode::Tab => app.next_field(),
        KeyCode::Enter => {
            if app.focused_field + 1 < app.fields.len() {
                app.next_field();
            } else {
                execute_modal_action(app, rpc_client, keypair).await;
            }
        }
        KeyCode::Backspace => app.current_field_backspace(),
        KeyCode::Char(c) => app.current_field_input(c),
        _ => {}
    }
}

// ─── Status fetch ─────────────────────────────────────────────────────────────

async fn fetch_status(app: &mut App, rpc_client: &RpcClient, keypair: &Keypair) {
    let Some(mint_str) = app.mint_address.clone() else {
        app.push_msg("No mint set — create a token first", RED);
        app.loading = false;
        return;
    };

    app.loading = true;
    app.loading_text = "Fetching status...".to_string();

    let program_id = match Pubkey::from_str(&crate::signer::get_program_id()) {
        Ok(p) => p,
        Err(e) => {
            app.push_msg(format!("✗ Invalid program ID: {}", e), RED);
            app.loading = false;
            return;
        }
    };

    let mint_pubkey = match Pubkey::from_str(&mint_str) {
        Ok(p) => p,
        Err(e) => {
            app.push_msg(format!("✗ Invalid mint: {}", e), RED);
            app.loading = false;
            return;
        }
    };

    let (config_pda, _) = Pubkey::find_program_address(
        &[b"stablecoin", &mint_pubkey.to_bytes()],
        &program_id,
    );
    let (compliance_pda, _) = Pubkey::find_program_address(
        &[b"compliance", &config_pda.to_bytes()],
        &program_id,
    );
    let (privacy_pda, _) = Pubkey::find_program_address(
        &[b"privacy", &config_pda.to_bytes()],
        &program_id,
    );

    let mut paused = false;
    if let Ok(response) = rpc_client.get_account(&config_pda.to_string()).await {
        if let Some(data_arr) = response
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.get("data"))
            .and_then(|d| d.as_array())
        {
            if let Some(encoded) = data_arr.first().and_then(|v| v.as_str()) {
                use base64::Engine;
                if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(encoded) {
                    // paused at offset 72: disc(8) + master_authority(32) + mint(32)
                    if decoded.len() > 72 {
                        paused = decoded[72] != 0;
                    }
                }
            }
        }
    }

    let supply_str = match rpc_client.get_token_supply(&mint_str).await {
        Ok(resp) => resp
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.get("uiAmountString"))
            .and_then(|v| v.as_str())
            .unwrap_or("0")
            .to_string(),
        Err(_) => "unknown".to_string(),
    };

    let compliance_exists = rpc_client
        .get_account(&compliance_pda.to_string())
        .await
        .ok()
        .and_then(|r| r.get("result")?.get("value").cloned())
        .map(|v| !v.is_null())
        .unwrap_or(false);

    let privacy_exists = rpc_client
        .get_account(&privacy_pda.to_string())
        .await
        .ok()
        .and_then(|r| r.get("result")?.get("value").cloned())
        .map(|v| !v.is_null())
        .unwrap_or(false);

    app.paused = paused;
    app.compliance_attached = compliance_exists;
    app.privacy_attached = privacy_exists;
    app.supply = Some(supply_str.clone());

    let tier = match (compliance_exists, privacy_exists) {
        (_, true) => "SSS-3 (Privacy)",
        (true, false) => "SSS-2 (Compliance)",
        _ => "SSS-1 (Basic)",
    };

    app.status_lines = vec![
        format!("Wallet:     {}", keypair.pubkey()),
        format!("Mint:       {}", mint_str),
        format!("Config:     {}", config_pda),
        format!("Tier:       {}", tier),
        format!("Supply:     {}", supply_str),
        format!("Paused:     {}", if paused { "Yes ⚠" } else { "No ✓" }),
        format!(
            "Compliance: {}",
            if compliance_exists {
                format!("Attached ({}...)", &compliance_pda.to_string()[..8])
            } else {
                "Not attached".to_string()
            }
        ),
        format!(
            "Privacy:    {}",
            if privacy_exists {
                format!("Attached ({}...)", &privacy_pda.to_string()[..8])
            } else {
                "Not attached".to_string()
            }
        ),
    ];

    app.loading = false;
    app.push_msg("✓ Status refreshed", GREEN);
}

// ─── Actions ──────────────────────────────────────────────────────────────────

async fn execute_modal_action(app: &mut App, rpc_client: &RpcClient, keypair: &Keypair) {
    let modal = app.modal.clone();
    let fields = app.fields.clone();
    let mint = app.mint_address.clone();

    app.modal = ActionModal::None;
    app.input_mode = InputMode::Normal;
    app.loading = true;

    match modal {
        ActionModal::Init => {
            let name = fields.get(0).map(|f| f.value.clone()).filter(|s| !s.is_empty());
            let symbol = fields.get(1).map(|f| f.value.clone()).filter(|s| !s.is_empty());
            let decimals: Option<u8> = fields.get(2).and_then(|f| f.value.parse().ok());
            let supply_cap: Option<u64> = fields.get(3).and_then(|f| f.value.parse().ok());
            app.loading_text = "Creating token...".to_string();

            let buf = gag::BufferRedirect::stdout().ok();
            let result = crate::commands::init::execute(
                rpc_client, keypair,
                name, symbol, decimals, supply_cap,
                None, None, None,
            ).await;
            drop(buf);

            match result {
                Ok(_) => {
                    if let Ok(cfg) = crate::config::CliConfig::load() {
                        if let Some(new_mint) = cfg.get_mint() {
                            app.mint_address = Some(new_mint.clone());
                            app.push_msg(
                                format!("✓ Token created: {}...", &new_mint[..8.min(new_mint.len())]),
                                GREEN,
                            );
                            fetch_status(app, rpc_client, keypair).await;
                            app.select_menu(1); // jump to Status after creation
                        }
                    }
                }
                Err(e) => app.push_msg(format!("✗ Init failed: {}", e), RED),
            }
        }

        ActionModal::Mint => {
            let recipient = fields.get(0).map(|f| f.value.clone()).unwrap_or_default();
            let amount_str = fields.get(1).map(|f| f.value.clone()).unwrap_or_default();
            let amount: u64 = amount_str.parse().unwrap_or(0);
            app.loading_text = format!("Minting {} tokens...", amount_str);

            let buf = gag::BufferRedirect::stdout().ok();
            let result = crate::commands::mint::execute(rpc_client, keypair, &recipient, amount, mint).await;
            drop(buf);

            match result {
                Ok(_) => app.push_msg(
                    format!("✓ Minted {} to {}...", amount_str, &recipient[..8.min(recipient.len())]),
                    GREEN,
                ),
                Err(e) => app.push_msg(format!("✗ Mint failed: {}", e), RED),
            }
        }

        ActionModal::Burn => {
            let amount_str = fields.get(0).map(|f| f.value.clone()).unwrap_or_default();
            let amount: u64 = amount_str.parse().unwrap_or(0);
            app.loading_text = format!("Burning {} tokens...", amount_str);

            let buf = gag::BufferRedirect::stdout().ok();
            let result = crate::commands::burn::execute(rpc_client, keypair, amount, mint).await;
            drop(buf);

            match result {
                Ok(_) => app.push_msg(format!("✓ Burned {} tokens", amount_str), ORANGE),
                Err(e) => app.push_msg(format!("✗ Burn failed: {}", e), RED),
            }
        }

        ActionModal::Freeze => {
            let address = fields.get(0).map(|f| f.value.clone()).unwrap_or_default();
            app.loading_text = format!("Freezing {}...", &address[..8.min(address.len())]);

            let buf = gag::BufferRedirect::stdout().ok();
            let result = crate::commands::freeze::execute(rpc_client, keypair, &address, mint).await;
            drop(buf);

            match result {
                Ok(_) => app.push_msg(format!("✓ Frozen: {}...", &address[..8.min(address.len())]), CYAN),
                Err(e) => app.push_msg(format!("✗ Freeze failed: {}", e), RED),
            }
        }

        ActionModal::Thaw => {
            let address = fields.get(0).map(|f| f.value.clone()).unwrap_or_default();
            app.loading_text = format!("Thawing {}...", &address[..8.min(address.len())]);

            let buf = gag::BufferRedirect::stdout().ok();
            let result = crate::commands::thaw::execute(rpc_client, keypair, &address, mint).await;
            drop(buf);

            match result {
                Ok(_) => app.push_msg(format!("✓ Thawed: {}...", &address[..8.min(address.len())]), GREEN),
                Err(e) => app.push_msg(format!("✗ Thaw failed: {}", e), RED),
            }
        }

        ActionModal::BlacklistAdd => {
            let address = fields.get(0).map(|f| f.value.clone()).unwrap_or_default();
            let reason = fields.get(1).map(|f| f.value.clone()).filter(|s| !s.is_empty());
            app.loading_text = format!("Blacklisting {}...", &address[..8.min(address.len())]);

            let buf = gag::BufferRedirect::stdout().ok();
            let result = crate::commands::blacklist::execute(
                rpc_client, keypair,
                crate::commands::blacklist::BlacklistAction::Add {
                    address: address.clone(),
                    reason,
                },
                mint,
            ).await;
            drop(buf);

            match result {
                Ok(_) => {
                    app.blacklist_entries.push(format!(
                        "{}...{}",
                        &address[..6.min(address.len())],
                        &address[address.len().saturating_sub(4)..]
                    ));
                    app.push_msg(format!("✓ Blacklisted: {}...", &address[..8.min(address.len())]), RED);
                }
                Err(e) => app.push_msg(format!("✗ Blacklist failed: {}", e), RED),
            }
        }

        ActionModal::BlacklistRemove => {
            let address = fields.get(0).map(|f| f.value.clone()).unwrap_or_default();
            app.loading_text = format!("Removing {} from blacklist...", &address[..8.min(address.len())]);

            let buf = gag::BufferRedirect::stdout().ok();
            let result = crate::commands::blacklist::execute(
                rpc_client, keypair,
                crate::commands::blacklist::BlacklistAction::Remove {
                    address: address.clone(),
                },
                mint,
            ).await;
            drop(buf);

            match result {
                Ok(_) => {
                    let short = format!(
                        "{}...{}",
                        &address[..6.min(address.len())],
                        &address[address.len().saturating_sub(4)..]
                    );
                    app.blacklist_entries.retain(|e| !e.starts_with(&short));
                    app.push_msg(format!("✓ Removed: {}...", &address[..8.min(address.len())]), GREEN);
                }
                Err(e) => app.push_msg(format!("✗ Remove failed: {}", e), RED),
            }
        }

        ActionModal::Seize => {
            let from = fields.get(0).map(|f| f.value.clone()).unwrap_or_default();
            let to = fields.get(1).map(|f| f.value.clone()).unwrap_or_default();
            let amount_str = fields.get(2).map(|f| f.value.clone()).unwrap_or_default();
            let amount: u64 = amount_str.parse().unwrap_or(0);
            app.loading_text = format!("Seizing {} tokens...", amount_str);

            let buf = gag::BufferRedirect::stdout().ok();
            let result = crate::commands::seize::execute(rpc_client, keypair, &from, &to, amount, mint).await;
            drop(buf);

            match result {
                Ok(_) => app.push_msg(
                    format!("✓ Seized {} tokens from {}...", amount_str, &from[..8.min(from.len())]),
                    ORANGE,
                ),
                Err(e) => app.push_msg(format!("✗ Seize failed: {}", e), RED),
            }
        }

        ActionModal::AddMinter => {
            let address = fields.get(0).map(|f| f.value.clone()).unwrap_or_default();
            app.loading_text = format!("Adding minter {}...", &address[..8.min(address.len())]);

            let buf = gag::BufferRedirect::stdout().ok();
            let result = crate::commands::minters::execute(
                rpc_client, keypair,
                crate::commands::minters::MinterAction::Add { address: address.clone() },
                mint,
            ).await;
            drop(buf);

            match result {
                Ok(_) => {
                    app.minter_entries.push(format!(
                        "{}...{}",
                        &address[..6.min(address.len())],
                        &address[address.len().saturating_sub(4)..]
                    ));
                    app.push_msg(format!("✓ Minter added: {}...", &address[..8.min(address.len())]), GREEN);
                }
                Err(e) => app.push_msg(format!("✗ Add minter failed: {}", e), RED),
            }
        }

        ActionModal::RemoveMinter => {
            let address = fields.get(0).map(|f| f.value.clone()).unwrap_or_default();
            app.loading_text = format!("Removing minter {}...", &address[..8.min(address.len())]);

            let buf = gag::BufferRedirect::stdout().ok();
            let result = crate::commands::minters::execute(
                rpc_client, keypair,
                crate::commands::minters::MinterAction::Remove { address: address.clone() },
                mint,
            ).await;
            drop(buf);

            match result {
                Ok(_) => {
                    let short = format!(
                        "{}...{}",
                        &address[..6.min(address.len())],
                        &address[address.len().saturating_sub(4)..]
                    );
                    app.minter_entries.retain(|e| !e.starts_with(&short));
                    app.push_msg(
                        format!("✓ Minter removed: {}...", &address[..8.min(address.len())]),
                        ORANGE,
                    );
                }
                Err(e) => app.push_msg(format!("✗ Remove minter failed: {}", e), RED),
            }
        }

        ActionModal::AttachCompliance => {
            let blacklister = fields.get(0).map(|f| f.value.clone()).unwrap_or_default();
            app.loading_text = "Attaching compliance module...".to_string();

            let buf = gag::BufferRedirect::stdout().ok();
            let result = crate::commands::compliance::attach(rpc_client, keypair, &blacklister, mint).await;
            drop(buf);

            match result {
                Ok(_) => {
                    app.compliance_attached = true;
                    app.push_msg("✓ Compliance module attached (SSS-2)", CYAN);
                }
                Err(e) => app.push_msg(format!("✗ Attach compliance failed: {}", e), RED),
            }
        }

        ActionModal::DetachCompliance => {
            app.loading_text = "Detaching compliance module...".to_string();

            let buf = gag::BufferRedirect::stdout().ok();
            let result = crate::commands::compliance::detach(rpc_client, keypair, mint).await;
            drop(buf);

            match result {
                Ok(_) => {
                    app.compliance_attached = false;
                    app.push_msg("✓ Compliance module detached", ORANGE);
                }
                Err(e) => app.push_msg(format!("✗ Detach compliance failed: {}", e), RED),
            }
        }

        ActionModal::AttachPrivacy => {
            let auth = fields.get(0).map(|f| f.value.clone()).unwrap_or_default();
            app.loading_text = "Attaching privacy module...".to_string();

            let buf = gag::BufferRedirect::stdout().ok();
            let result = crate::commands::privacy::attach(rpc_client, keypair, &auth, false, mint).await;
            drop(buf);

            match result {
                Ok(_) => {
                    app.privacy_attached = true;
                    app.push_msg("✓ Privacy module attached (SSS-3)", PURPLE);
                }
                Err(e) => app.push_msg(format!("✗ Attach privacy failed: {}", e), RED),
            }
        }

        ActionModal::DetachPrivacy => {
            app.loading_text = "Detaching privacy module...".to_string();

            let buf = gag::BufferRedirect::stdout().ok();
            let result = crate::commands::privacy::detach(rpc_client, keypair, mint).await;
            drop(buf);

            match result {
                Ok(_) => {
                    app.privacy_attached = false;
                    app.push_msg("✓ Privacy module detached", ORANGE);
                }
                Err(e) => app.push_msg(format!("✗ Detach privacy failed: {}", e), RED),
            }
        }

        ActionModal::AllowlistAdd => {
            let address = fields.get(0).map(|f| f.value.clone()).unwrap_or_default();
            app.loading_text = format!("Adding {} to allowlist...", &address[..8.min(address.len())]);

            let buf = gag::BufferRedirect::stdout().ok();
            let result = crate::commands::privacy::allowlist_add(rpc_client, keypair, &address, mint).await;
            drop(buf);

            match result {
                Ok(_) => app.push_msg(
                    format!("✓ Allowlisted: {}...", &address[..8.min(address.len())]),
                    PURPLE,
                ),
                Err(e) => app.push_msg(format!("✗ Allowlist add failed: {}", e), RED),
            }
        }

        ActionModal::AllowlistRemove => {
            let address = fields.get(0).map(|f| f.value.clone()).unwrap_or_default();
            app.loading_text = format!("Removing {} from allowlist...", &address[..8.min(address.len())]);

            let buf = gag::BufferRedirect::stdout().ok();
            let result = crate::commands::privacy::allowlist_remove(rpc_client, keypair, &address, mint).await;
            drop(buf);

            match result {
                Ok(_) => app.push_msg(
                    format!("✓ Removed from allowlist: {}...", &address[..8.min(address.len())]),
                    PURPLE,
                ),
                Err(e) => app.push_msg(format!("✗ Allowlist remove failed: {}", e), RED),
            }
        }

        ActionModal::None => {}
    }

    app.loading = false;
    app.fields.clear();
}

// ─── UI ───────────────────────────────────────────────────────────────────────

fn ui(f: &mut Frame, app: &App) {
    let area = f.area();
    f.render_widget(Block::default().style(Style::default().bg(DARK)), area);

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(3)])
        .split(area);

    render_header(f, app, outer[0]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(24), Constraint::Min(0)])
        .split(outer[1]);

    render_sidebar(f, app, body[0]);
    render_main(f, app, body[1]);
    render_footer(f, app, outer[2]);

    if app.modal != ActionModal::None {
        render_modal(f, app, area);
    }
    if app.loading {
        render_loading(f, app, area);
    }
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let spinner_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let spinner = spinner_chars[(app.tick_count as usize / 2) % spinner_chars.len()];

    let tier_color = match app.tier() {
        "SSS-3" => PURPLE,
        "SSS-2" => CYAN,
        _ => DIM,
    };

    let mint_display = app.mint_address.as_deref()
        .map(|m| {
            if m.len() >= 10 {
                format!("Mint: {}...{}", &m[..6], &m[m.len() - 4..])
            } else {
                format!("Mint: {}", m)
            }
        })
        .unwrap_or_else(|| "No mint — create one first".to_string());

    let title = Line::from(vec![
        Span::styled(" SSS ", Style::default().fg(DARK).bg(CYAN).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled("SOLANA STABLECOIN STANDARD", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(spinner, Style::default().fg(DIM)),
        Span::raw("  "),
        Span::styled(mint_display, Style::default().fg(DIM)),
        Span::raw("  "),
        Span::styled(
            format!("[{}]", app.tier()),
            Style::default().fg(tier_color).add_modifier(Modifier::BOLD),
        ),
    ]);

    let para = Paragraph::new(title)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(CYAN))
                .style(Style::default().bg(PANEL)),
        )
        .alignment(Alignment::Left);
    f.render_widget(para, area);
}

fn render_sidebar(f: &mut Frame, app: &App, area: Rect) {
    let sections = Section::all();
    let items: Vec<ListItem> = sections.iter().enumerate().map(|(i, s)| {
        let selected = app.menu_state.selected() == Some(i);
        let icon = match s {
            Section::Init => "✚",
            Section::Status => "◈",
            Section::MintBurn => "◎",
            Section::FreezeThaw => "❄",
            Section::Blacklist => "⛔",
            Section::Minters => "✦",
            Section::Modules => "⬡",
        };
        let line = Line::from(vec![
            Span::raw(" "),
            Span::styled(icon, Style::default().fg(if selected { DARK } else { CYAN })),
            Span::raw("  "),
            Span::styled(
                s.title(),
                Style::default()
                    .fg(if selected { DARK } else { Color::White })
                    .add_modifier(if selected { Modifier::BOLD } else { Modifier::empty() }),
            ),
        ]);
        if selected {
            ListItem::new(line).style(Style::default().bg(CYAN))
        } else {
            ListItem::new(line)
        }
    }).collect();

    let list = List::new(items).block(
        Block::default()
            .title(Span::styled(" MENU ", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(DIM))
            .style(Style::default().bg(PANEL)),
    );
    let mut state = app.menu_state.clone();
    f.render_stateful_widget(list, area, &mut state);
}

fn render_main(f: &mut Frame, app: &App, area: Rect) {
    match app.selected_section {
        Section::Init => render_init(f, app, area),
        Section::Status => render_status(f, app, area),
        Section::MintBurn => render_mint_burn(f, app, area),
        Section::FreezeThaw => render_freeze_thaw(f, app, area),
        Section::Blacklist => render_blacklist(f, app, area),
        Section::Minters => render_minters(f, app, area),
        Section::Modules => render_modules(f, app, area),
    }
}

fn panel_block(title: &str, border_color: Color) -> Block<'static> {
    Block::default()
        .title(Span::styled(
            format!(" {} ", title),
            Style::default().fg(border_color).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(PANEL))
        .padding(Padding::horizontal(1))
}

fn render_init(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(10)])
        .split(area);

    let content = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "  CREATE NEW SSS TOKEN",
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            "  ─────────────────────────────────────",
            Style::default().fg(DIM),
        )]),
        Line::from(vec![
            Span::styled("  [N]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("  Create a new SSS-1 token", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Tokens start as SSS-1 (basic).",
            Style::default().fg(DIM),
        )]),
        Line::from(vec![Span::styled(
            "  Use the Modules section to upgrade:",
            Style::default().fg(DIM),
        )]),
        Line::from(vec![Span::styled(
            "  → Attach compliance module = SSS-2",
            Style::default().fg(DIM),
        )]),
        Line::from(vec![Span::styled(
            "  → Attach privacy module    = SSS-3",
            Style::default().fg(DIM),
        )]),
    ];

    let para = Paragraph::new(content)
        .block(panel_block("CREATE TOKEN", CYAN))
        .wrap(Wrap { trim: false });
    f.render_widget(para, chunks[0]);
    render_messages(f, app, chunks[1]);
}

fn render_status(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(10)])
        .split(area);

    let content: Vec<Line> = if app.status_lines.is_empty() {
        vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "  Press R to fetch status",
                Style::default().fg(DIM),
            )]),
        ]
    } else {
        app.status_lines.iter().map(|l| {
            let parts: Vec<&str> = l.splitn(2, ':').collect();
            if parts.len() == 2 {
                Line::from(vec![
                    Span::styled(
                        format!("  {:<12}", parts[0]),
                        Style::default().fg(DIM),
                    ),
                    Span::styled(": ", Style::default().fg(DIM)),
                    Span::styled(
                        parts[1].trim().to_string(),
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                    ),
                ])
            } else {
                Line::from(Span::raw(l.clone()))
            }
        }).collect()
    };

    let para = Paragraph::new(content)
        .block(panel_block("STATUS / SUPPLY", CYAN))
        .wrap(Wrap { trim: false });
    f.render_widget(para, chunks[0]);
    render_messages(f, app, chunks[1]);
}

fn render_mint_burn(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(10)])
        .split(area);

    let content = vec![
        Line::from(""),
        Line::from(vec![Span::styled("  MINT", Style::default().fg(GREEN).add_modifier(Modifier::BOLD))]),
        Line::from(vec![Span::styled("  ─────────────────────────────────────", Style::default().fg(DIM))]),
        Line::from(vec![
            Span::styled("  [M]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("  Mint tokens to a recipient address", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled("  BURN", Style::default().fg(ORANGE).add_modifier(Modifier::BOLD))]),
        Line::from(vec![Span::styled("  ─────────────────────────────────────", Style::default().fg(DIM))]),
        Line::from(vec![
            Span::styled("  [B]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("  Burn tokens from your account", Style::default().fg(Color::White)),
        ]),
    ];

    let para = Paragraph::new(content)
        .block(panel_block("MINT / BURN", GREEN))
        .wrap(Wrap { trim: false });
    f.render_widget(para, chunks[0]);
    render_messages(f, app, chunks[1]);
}

fn render_freeze_thaw(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(10)])
        .split(area);

    let content = vec![
        Line::from(""),
        Line::from(vec![Span::styled("  FREEZE", Style::default().fg(CYAN).add_modifier(Modifier::BOLD))]),
        Line::from(vec![Span::styled("  ─────────────────────────────────────", Style::default().fg(DIM))]),
        Line::from(vec![
            Span::styled("  [F]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("  Freeze a wallet's token account", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled("  THAW", Style::default().fg(GREEN).add_modifier(Modifier::BOLD))]),
        Line::from(vec![Span::styled("  ─────────────────────────────────────", Style::default().fg(DIM))]),
        Line::from(vec![
            Span::styled("  [T]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("  Thaw a frozen account", Style::default().fg(Color::White)),
        ]),
    ];

    let para = Paragraph::new(content)
        .block(panel_block("FREEZE / THAW", CYAN))
        .wrap(Wrap { trim: false });
    f.render_widget(para, chunks[0]);
    render_messages(f, app, chunks[1]);
}

fn render_blacklist(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(10)])
        .split(area);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(chunks[0]);

    let compliance_note = if !app.compliance_attached {
        "  ⚠ Compliance module not attached"
    } else {
        ""
    };

    let actions = vec![
        Line::from(""),
        Line::from(vec![Span::styled(compliance_note, Style::default().fg(ORANGE))]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [A]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("  Add to blacklist", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [R]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("  Remove from blacklist", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [S]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("  Seize tokens", Style::default().fg(Color::White)),
        ]),
    ];

    let para = Paragraph::new(actions)
        .block(panel_block("ACTIONS", RED))
        .wrap(Wrap { trim: false });
    f.render_widget(para, main_chunks[0]);

    let items: Vec<ListItem> = if app.blacklist_entries.is_empty() {
        vec![ListItem::new(Line::from(vec![Span::styled(
            "  No entries",
            Style::default().fg(DIM),
        )]))]
    } else {
        app.blacklist_entries.iter().map(|e| {
            ListItem::new(Line::from(vec![
                Span::styled("  ⛔ ", Style::default().fg(RED)),
                Span::styled(e.clone(), Style::default().fg(Color::White)),
            ]))
        }).collect()
    };

    let list = List::new(items)
        .block(panel_block("BLACKLISTED ADDRESSES", RED))
        .highlight_style(Style::default().bg(Color::Rgb(40, 10, 10)));
    let mut state = app.list_state.clone();
    f.render_stateful_widget(list, main_chunks[1], &mut state);
    render_messages(f, app, chunks[1]);
}

fn render_minters(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(10)])
        .split(area);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(chunks[0]);

    let actions = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  [A]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("  Add new minter", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [R]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("  Remove minter", Style::default().fg(Color::White)),
        ]),
    ];

    let para = Paragraph::new(actions)
        .block(panel_block("ACTIONS", GREEN))
        .wrap(Wrap { trim: false });
    f.render_widget(para, main_chunks[0]);

    let items: Vec<ListItem> = if app.minter_entries.is_empty() {
        vec![ListItem::new(Line::from(vec![Span::styled(
            "  No minters added",
            Style::default().fg(DIM),
        )]))]
    } else {
        app.minter_entries.iter().map(|e| {
            ListItem::new(Line::from(vec![
                Span::styled("  ✦ ", Style::default().fg(GREEN)),
                Span::styled(e.clone(), Style::default().fg(Color::White)),
            ]))
        }).collect()
    };

    let list = List::new(items)
        .block(panel_block("MINTERS", GREEN))
        .highlight_style(Style::default().bg(Color::Rgb(10, 40, 10)));
    let mut state = app.list_state.clone();
    f.render_stateful_widget(list, main_chunks[1], &mut state);
    render_messages(f, app, chunks[1]);
}

fn render_modules(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(10)])
        .split(area);

    let compliance_status = if app.compliance_attached {
        Span::styled(
            "● Attached (SSS-2)",
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled("○ Not attached", Style::default().fg(DIM))
    };

    let privacy_status = if app.privacy_attached {
        Span::styled(
            "● Attached (SSS-3)",
            Style::default().fg(PURPLE).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled("○ Not attached", Style::default().fg(DIM))
    };

    let content = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  COMPLIANCE MODULE  ", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            compliance_status,
        ]),
        Line::from(vec![Span::styled("  ─────────────────────────────────────", Style::default().fg(DIM))]),
        Line::from(vec![
            Span::styled("  [1]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("  Attach compliance module (→ SSS-2)", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  [2]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("  Detach compliance module (→ SSS-1)", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  PRIVACY MODULE  ", Style::default().fg(PURPLE).add_modifier(Modifier::BOLD)),
            privacy_status,
        ]),
        Line::from(vec![Span::styled("  ─────────────────────────────────────", Style::default().fg(DIM))]),
        Line::from(vec![
            Span::styled("  [3]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("  Attach privacy module (→ SSS-3)", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  [4]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("  Detach privacy module", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ALLOWLIST  ", Style::default().fg(PURPLE).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![Span::styled("  ─────────────────────────────────────", Style::default().fg(DIM))]),
        Line::from(vec![
            Span::styled("  [5]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("  Add address to allowlist", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  [6]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("  Remove address from allowlist", Style::default().fg(Color::White)),
        ]),
    ];

    let para = Paragraph::new(content)
        .block(panel_block("MODULE MANAGEMENT", PURPLE))
        .wrap(Wrap { trim: false });
    f.render_widget(para, chunks[0]);
    render_messages(f, app, chunks[1]);
}

fn render_messages(f: &mut Frame, app: &App, area: Rect) {
    let content: Vec<Line> = if app.messages.is_empty() {
        vec![Line::from(vec![Span::styled(
            "  No recent activity",
            Style::default().fg(DIM),
        )])]
    } else {
        app.messages.iter().rev().take(5).map(|(msg, color)| {
            Line::from(vec![
                Span::styled("  › ", Style::default().fg(*color)),
                Span::styled(msg.clone(), Style::default().fg(Color::White)),
            ])
        }).collect()
    };

    let para = Paragraph::new(content)
        .block(panel_block("ACTIVITY LOG", DIM))
        .wrap(Wrap { trim: true });
    f.render_widget(para, area);
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let shortcuts = match app.selected_section {
        Section::Init => " [N] New Token   [↑↓] Navigate   [Q] Quit",
        Section::Status => " [R] Refresh   [↑↓] Navigate   [Q] Quit",
        Section::MintBurn => " [M] Mint   [B] Burn   [↑↓] Navigate   [Q] Quit",
        Section::FreezeThaw => " [F] Freeze   [T] Thaw   [↑↓] Navigate   [Q] Quit",
        Section::Blacklist => " [A] Add   [R] Remove   [S] Seize   [↑↓] Navigate   [Q] Quit",
        Section::Minters => " [A] Add   [R] Remove   [↑↓] Navigate   [Q] Quit",
        Section::Modules => " [1-6] Actions   [↑↓] Navigate   [Q] Quit",
    };

    let wallet_short = if app.wallet.len() > 12 {
        format!("{}...{}", &app.wallet[..6], &app.wallet[app.wallet.len() - 4..])
    } else {
        app.wallet.clone()
    };

    let line = Line::from(vec![
        Span::styled(shortcuts, Style::default().fg(DIM)),
        Span::raw("   "),
        Span::styled("Wallet: ", Style::default().fg(DIM)),
        Span::styled(wallet_short, Style::default().fg(CYAN)),
    ]);

    let para = Paragraph::new(line).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(DIM))
            .style(Style::default().bg(PANEL)),
    );
    f.render_widget(para, area);
}

fn render_modal(f: &mut Frame, app: &App, area: Rect) {
    let title = match &app.modal {
        ActionModal::Init => "CREATE TOKEN",
        ActionModal::Mint => "MINT TOKENS",
        ActionModal::Burn => "BURN TOKENS",
        ActionModal::Freeze => "FREEZE ACCOUNT",
        ActionModal::Thaw => "THAW ACCOUNT",
        ActionModal::BlacklistAdd => "ADD TO BLACKLIST",
        ActionModal::BlacklistRemove => "REMOVE FROM BLACKLIST",
        ActionModal::Seize => "SEIZE TOKENS",
        ActionModal::AddMinter => "ADD MINTER",
        ActionModal::RemoveMinter => "REMOVE MINTER",
        ActionModal::AttachCompliance => "ATTACH COMPLIANCE MODULE",
        ActionModal::DetachCompliance => "DETACH COMPLIANCE MODULE",
        ActionModal::AttachPrivacy => "ATTACH PRIVACY MODULE",
        ActionModal::DetachPrivacy => "DETACH PRIVACY MODULE",
        ActionModal::AllowlistAdd => "ADD TO ALLOWLIST",
        ActionModal::AllowlistRemove => "REMOVE FROM ALLOWLIST",
        ActionModal::None => "",
    };

    let border_color = match &app.modal {
        ActionModal::Init => CYAN,
        ActionModal::Mint => GREEN,
        ActionModal::Burn => ORANGE,
        ActionModal::Freeze | ActionModal::Thaw => CYAN,
        ActionModal::BlacklistAdd | ActionModal::BlacklistRemove | ActionModal::Seize => RED,
        ActionModal::AddMinter | ActionModal::RemoveMinter => GREEN,
        ActionModal::AttachCompliance | ActionModal::DetachCompliance => CYAN,
        ActionModal::AttachPrivacy
        | ActionModal::DetachPrivacy
        | ActionModal::AllowlistAdd
        | ActionModal::AllowlistRemove => PURPLE,
        ActionModal::None => CYAN,
    };

    let height = if app.fields.is_empty() {
        8u16
    } else {
        6 + (app.fields.len() as u16 * 4)
    };

    let modal_area = centered_rect(60, height, area);
    f.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(Span::styled(
            format!(" {} ", title),
            Style::default().fg(border_color).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(PANEL));
    f.render_widget(block, modal_area);

    let inner = Rect {
        x: modal_area.x + 2,
        y: modal_area.y + 2,
        width: modal_area.width.saturating_sub(4),
        height: modal_area.height.saturating_sub(4),
    };

    // Confirm-only modals (DetachCompliance, DetachPrivacy)
    if app.fields.is_empty() {
        let confirm_area = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 2,
        };
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("Press ", Style::default().fg(DIM)),
                Span::styled(
                    "[Enter]",
                    Style::default().fg(border_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" to confirm or ", Style::default().fg(DIM)),
                Span::styled("[Esc]", Style::default().fg(DIM)),
                Span::styled(" to cancel.", Style::default().fg(DIM)),
            ])),
            confirm_area,
        );
    }

    let mut y_offset = inner.y;
    for (i, field) in app.fields.iter().enumerate() {
        let focused = app.focused_field == i;
        let label_area = Rect {
            x: inner.x,
            y: y_offset,
            width: inner.width,
            height: 1,
        };
        let input_area = Rect {
            x: inner.x,
            y: y_offset + 1,
            width: inner.width,
            height: 2,
        };

        f.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                field.label.clone(),
                Style::default()
                    .fg(if focused { border_color } else { DIM })
                    .add_modifier(Modifier::BOLD),
            )])),
            label_area,
        );

        let display = if field.value.is_empty() {
            Span::styled(field.placeholder.clone(), Style::default().fg(DIM))
        } else {
            Span::styled(
                format!("{}{}", field.value, if focused { "█" } else { "" }),
                Style::default().fg(Color::White),
            )
        };

        let input_para = Paragraph::new(Line::from(vec![display])).block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(if focused { border_color } else { DIM })),
        );
        f.render_widget(input_para, input_area);
        y_offset += 4;
    }

    let hint_area = Rect {
        x: inner.x,
        y: modal_area.y + modal_area.height - 2,
        width: inner.width,
        height: 1,
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            "[Tab] Next field   [Enter] Submit   [Esc] Cancel",
            Style::default().fg(DIM),
        )])),
        hint_area,
    );
}

fn render_loading(f: &mut Frame, app: &App, area: Rect) {
    let spinner_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let spinner = spinner_chars[(app.tick_count as usize / 2) % spinner_chars.len()];
    let loading_area = centered_rect(40, 3, area);
    f.render_widget(Clear, loading_area);

    let para = Paragraph::new(Line::from(vec![
        Span::styled(format!(" {} ", spinner), Style::default().fg(CYAN)),
        Span::styled(app.loading_text.clone(), Style::default().fg(Color::White)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(CYAN))
            .style(Style::default().bg(PANEL)),
    )
    .alignment(Alignment::Center);
    f.render_widget(para, loading_area);
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}
