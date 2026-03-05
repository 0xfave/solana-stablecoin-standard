use std::io;
use gag::BufferRedirect;
use std::time::{Duration, Instant};
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
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, BorderType, Clear, List, ListItem, ListState, Padding,
        Paragraph, Wrap,
    },
    Frame, Terminal,
};

use crate::rpc_client::RpcClient;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;

// ─── Colour palette ──────────────────────────────────────────────────────────

const CYAN: Color = Color::Rgb(37, 209, 244);
const DARK: Color = Color::Rgb(14, 14, 18);
const PANEL: Color = Color::Rgb(20, 20, 23);
const DIM: Color = Color::DarkGray;
const GREEN: Color = Color::Rgb(74, 222, 128);
const ORANGE: Color = Color::Rgb(251, 146, 60);
const RED: Color = Color::Rgb(248, 113, 113);
const YELLOW: Color = Color::Rgb(250, 204, 21);

// ─── App state ───────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
enum Section {
    Status,
    MintBurn,
    FreezeThaw,
    Blacklist,
    Minters,
}

impl Section {
    fn title(&self) -> &str {
        match self {
            Section::Status => "Status / Supply",
            Section::MintBurn => "Mint / Burn",
            Section::FreezeThaw => "Freeze / Thaw",
            Section::Blacklist => "Blacklist",
            Section::Minters => "Minters",
        }
    }

    fn all() -> Vec<Section> {
        vec![
            Section::Status,
            Section::MintBurn,
            Section::FreezeThaw,
            Section::Blacklist,
            Section::Minters,
        ]
    }
}

#[derive(Clone, PartialEq)]
enum InputMode {
    Normal,
    Editing,
}

#[derive(Clone, PartialEq)]
enum ActiveInput {
    None,
    Recipient,
    Amount,
    Address,
    Reason,
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
    Mint,
    Burn,
    Freeze,
    Thaw,
    BlacklistAdd,
    BlacklistRemove,
    AddMinter,
    RemoveMinter,
    Confirm(String),
}

struct App {
    // Navigation
    menu_state: ListState,
    selected_section: Section,

    // Status data
    mint_address: Option<String>,
    supply: Option<String>,
    paused: bool,
    wallet: String,
    status_lines: Vec<String>,

    // Input
    input_mode: InputMode,
    active_input: ActiveInput,
    fields: Vec<InputField>,
    focused_field: usize,

    // Modal
    modal: ActionModal,

    // Feedback
    messages: Vec<(String, Color)>,
    loading: bool,
    loading_text: String,

    // Blacklist / Minters lists
    blacklist_entries: Vec<String>,
    minter_entries: Vec<String>,
    list_state: ListState,

    // Tick
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
            selected_section: Section::Status,
            mint_address: mint,
            supply: None,
            paused: false,
            wallet,
            status_lines: Vec::new(),
            input_mode: InputMode::Normal,
            active_input: ActiveInput::None,
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
        let m = msg.into();
        self.messages.push((m, color));
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

    fn open_modal(&mut self, modal: ActionModal) {
        self.fields.clear();
        self.focused_field = 0;
        self.input_mode = InputMode::Editing;

        match &modal {
            ActionModal::Mint => {
                self.fields.push(InputField::new("Recipient", "Wallet address"));
                self.fields.push(InputField::new("Amount", "e.g. 1000.00"));
            }
            ActionModal::Burn => {
                self.fields.push(InputField::new("Amount", "e.g. 500.00"));
            }
            ActionModal::Freeze | ActionModal::Thaw => {
                self.fields.push(InputField::new("Address", "Wallet address to freeze/thaw"));
            }
            ActionModal::BlacklistAdd => {
                self.fields.push(InputField::new("Address", "Wallet address to blacklist"));
                self.fields.push(InputField::new("Reason", "e.g. suspicious activity"));
            }
            ActionModal::BlacklistRemove | ActionModal::RemoveMinter => {
                self.fields.push(InputField::new("Address", "Wallet address to remove"));
            }
            ActionModal::AddMinter => {
                self.fields.push(InputField::new("Address", "Minter wallet address"));
            }
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

pub async fn run(
    rpc_client: &RpcClient,
    keypair: &Keypair,
    mint: Option<String>,
) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let wallet = keypair.pubkey().to_string();
    let mut app = App::new(wallet, mint);

    // Initial status fetch
    app.loading = true;
    app.loading_text = "Fetching token status...".to_string();

    let result = run_loop(&mut terminal, &mut app, rpc_client, keypair).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
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

                // Global quit
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

async fn handle_normal_key(
    app: &mut App,
    key: KeyCode,
    rpc_client: &RpcClient,
    keypair: &Keypair,
) {
    match key {
        KeyCode::Up | KeyCode::Char('k') => app.menu_up(),
        KeyCode::Down | KeyCode::Char('j') => app.menu_down(),
        KeyCode::Esc => {
            app.modal = ActionModal::None;
        }

        // ✅ Section shortcuts FIRST before any global keys
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
        KeyCode::Char('a') if app.selected_section == Section::Minters => {
            app.open_modal(ActionModal::AddMinter);
        }
        KeyCode::Char('r') if app.selected_section == Section::Minters => {
            app.open_modal(ActionModal::RemoveMinter);
        }

        // ✅ Global refresh AFTER section shortcuts
        KeyCode::Char('r') => {
            fetch_status(app, rpc_client, keypair).await;
        }

        KeyCode::Enter if app.selected_section == Section::Status => {
            fetch_status(app, rpc_client, keypair).await;
        }

        _ => {}
    }
}

async fn handle_editing_key(
    app: &mut App,
    key: KeyCode,
    rpc_client: &RpcClient,
    keypair: &Keypair,
) {
    match key {
        KeyCode::Esc => {
            app.modal = ActionModal::None;
            app.input_mode = InputMode::Normal;
            app.fields.clear();
        }
        KeyCode::Tab => {
            app.next_field();
        }
        KeyCode::Enter => {
            if app.focused_field + 1 < app.fields.len() {
                app.next_field();
            } else {
                // Submit
                execute_modal_action(app, rpc_client, keypair).await;
            }
        }
        KeyCode::Backspace => {
            app.current_field_backspace();
        }
        KeyCode::Char(c) => {
            app.push_msg(format!("typed: {}", c), CYAN);
            app.current_field_input(c);
        }
        _ => {}
    }
}

// ─── Actions ──────────────────────────────────────────────────────────────────

async fn fetch_status(app: &mut App, rpc_client: &RpcClient, keypair: &Keypair) {
    let Some(mint) = app.mint_address.clone() else {
        app.push_msg("✗ No mint set — run init first", RED);
        return;
    };

    app.loading = true;
    app.loading_text = "Fetching status...".to_string();

    // ✅ Actually call the real commands
    match crate::commands::status::execute(rpc_client, Some(mint.clone())).await {
        Ok(_) => {}
        Err(e) => {
            app.push_msg(format!("✗ Status error: {}", e), RED);
            app.loading = false;
            return;
        }
    }

    match crate::commands::supply::execute(rpc_client, Some(mint.clone())).await {
        Ok(_) => {}
        Err(e) => {
            app.push_msg(format!("✗ Supply error: {}", e), RED);
            app.loading = false;
            return;
        }
    }

    // Update display
    app.status_lines = vec![
        format!("Wallet:  {}", keypair.pubkey()),
        format!("Mint:    {}", mint),
        format!("Paused:  {}", if app.paused { "Yes ⚠" } else { "No ✓" }),
    ];

    app.loading = false;
    app.push_msg("✓ Status refreshed", GREEN);
}

async fn execute_modal_action(
    app: &mut App,
    rpc_client: &RpcClient,
    keypair: &Keypair,
) {
    let modal = app.modal.clone();
    let fields = app.fields.clone();
    let mint = app.mint_address.clone();

    app.modal = ActionModal::None;
    app.input_mode = InputMode::Normal;
    app.loading = true;

    match modal {
        ActionModal::Mint => {
            let recipient = fields.get(0).map(|f| f.value.clone()).unwrap_or_default();
            let amount_str = fields.get(1).map(|f| f.value.clone()).unwrap_or_default();
            let amount: u64 = amount_str.parse().unwrap_or(0);
            app.loading_text = format!("Minting {} tokens...", amount_str);

            let buf = gag::BufferRedirect::stdout().ok();
            let result = crate::commands::mint::execute(
                rpc_client, keypair, &recipient, amount, mint,
            ).await;
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
            let result = crate::commands::burn::execute(
                rpc_client, keypair, amount, mint,
            ).await;
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
            let result = crate::commands::freeze::execute(
                rpc_client, keypair, &address, mint,
            ).await;
            drop(buf);

            match result {
                Ok(_) => app.push_msg(
                    format!("✓ Frozen: {}...", &address[..8.min(address.len())]),
                    CYAN,
                ),
                Err(e) => app.push_msg(format!("✗ Freeze failed: {}", e), RED),
            }
        }

        ActionModal::Thaw => {
            let address = fields.get(0).map(|f| f.value.clone()).unwrap_or_default();
            app.loading_text = format!("Thawing {}...", &address[..8.min(address.len())]);

            let buf = gag::BufferRedirect::stdout().ok();
            let result = crate::commands::thaw::execute(
                rpc_client, keypair, &address, mint,
            ).await;
            drop(buf);

            match result {
                Ok(_) => app.push_msg(
                    format!("✓ Thawed: {}...", &address[..8.min(address.len())]),
                    GREEN,
                ),
                Err(e) => app.push_msg(format!("✗ Thaw failed: {}", e), RED),
            }
        }

        ActionModal::BlacklistAdd => {
            let address = fields.get(0).map(|f| f.value.clone()).unwrap_or_default();
            let reason = fields.get(1).map(|f| f.value.clone()).filter(|s| !s.is_empty());
            app.loading_text = format!("Blacklisting {}...", &address[..8.min(address.len())]);

            let buf = gag::BufferRedirect::stdout().ok();
            let result = crate::commands::blacklist::execute(
                rpc_client,
                keypair,
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
                    app.push_msg(
                        format!("✓ Blacklisted: {}...", &address[..8.min(address.len())]),
                        RED,
                    );
                }
                Err(e) => app.push_msg(format!("✗ Blacklist failed: {}", e), RED),
            }
        }

        ActionModal::BlacklistRemove => {
            let address = fields.get(0).map(|f| f.value.clone()).unwrap_or_default();
            app.loading_text = format!("Removing {} from blacklist...", &address[..8.min(address.len())]);

            let buf = gag::BufferRedirect::stdout().ok();
            let result = crate::commands::blacklist::execute(
                rpc_client,
                keypair,
                crate::commands::blacklist::BlacklistAction::Remove {
                    address: address.clone(),
                },
                mint,
            ).await;
            drop(buf);

            match result {
                Ok(_) => {
                    // Remove from local list if present
                    let short = format!(
                        "{}...{}",
                        &address[..6.min(address.len())],
                        &address[address.len().saturating_sub(4)..]
                    );
                    app.blacklist_entries.retain(|e| !e.starts_with(&short));
                    app.push_msg(
                        format!("✓ Removed: {}...", &address[..8.min(address.len())]),
                        GREEN,
                    );
                }
                Err(e) => app.push_msg(format!("✗ Remove failed: {}", e), RED),
            }
        }

        ActionModal::AddMinter => {
            let address = fields.get(0).map(|f| f.value.clone()).unwrap_or_default();
            app.loading_text = format!("Adding minter {}...", &address[..8.min(address.len())]);

            let buf = gag::BufferRedirect::stdout().ok();
            let result = crate::commands::minters::execute(
                rpc_client,
                keypair,
                crate::commands::minters::MinterAction::Add {
                    address: address.clone(),
                },
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
                    app.push_msg(
                        format!("✓ Minter added: {}...", &address[..8.min(address.len())]),
                        GREEN,
                    );
                }
                Err(e) => app.push_msg(format!("✗ Add minter failed: {}", e), RED),
            }
        }

        ActionModal::RemoveMinter => {
            let address = fields.get(0).map(|f| f.value.clone()).unwrap_or_default();
            app.loading_text = format!("Removing minter {}...", &address[..8.min(address.len())]);

            let buf = gag::BufferRedirect::stdout().ok();
            let result = crate::commands::minters::execute(
                rpc_client,
                keypair,
                crate::commands::minters::MinterAction::Remove {
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
                    app.minter_entries.retain(|e| !e.starts_with(&short));
                    app.push_msg(
                        format!("✓ Minter removed: {}...", &address[..8.min(address.len())]),
                        ORANGE,
                    );
                }
                Err(e) => app.push_msg(format!("✗ Remove minter failed: {}", e), RED),
            }
        }

        _ => {}
    }

    app.loading = false;
    app.fields.clear();
}

// ─── UI ───────────────────────────────────────────────────────────────────────

fn ui(f: &mut Frame, app: &App) {
    let area = f.area();

    // Background
    f.render_widget(Block::default().style(Style::default().bg(DARK)), area);

    // Outer layout: header + body + footer
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // header
            Constraint::Min(0),     // body
            Constraint::Length(3),  // footer
        ])
        .split(area);

    render_header(f, app, outer[0]);

    // Body: sidebar + main
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(24), // sidebar
            Constraint::Min(0),     // main panel
        ])
        .split(outer[1]);

    render_sidebar(f, app, body[0]);
    render_main(f, app, body[1]);
    render_footer(f, app, outer[2]);

    // Overlay modal if active
    if app.modal != ActionModal::None {
        render_modal(f, app, area);
    }

    // Loading overlay
    if app.loading {
        render_loading(f, app, area);
    }
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let spinner_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let spinner = spinner_chars[(app.tick_count as usize / 2) % spinner_chars.len()];

    let title = Line::from(vec![
        Span::styled(" SSS ", Style::default().fg(DARK).bg(CYAN).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled("SOLANA STABLECOIN STANDARD", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(spinner, Style::default().fg(DIM)),
        Span::raw("  "),
        Span::styled(
            app.mint_address.as_deref().map(|m| format!("Mint: {}...{}", &m[..6], &m[m.len()-4..])).unwrap_or("No mint set".to_string()),
            Style::default().fg(DIM),
        ),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(CYAN))
        .style(Style::default().bg(PANEL));

    let para = Paragraph::new(title)
        .block(block)
        .alignment(Alignment::Left);

    f.render_widget(para, area);
}

fn render_sidebar(f: &mut Frame, app: &App, area: Rect) {
    let sections = Section::all();
    let items: Vec<ListItem> = sections
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let selected = app.menu_state.selected() == Some(i);
            let icon = match s {
                Section::Status => "◈",
                Section::MintBurn => "◎",
                Section::FreezeThaw => "❄",
                Section::Blacklist => "⛔",
                Section::Minters => "✦",
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
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(" MENU ", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(DIM))
        .style(Style::default().bg(PANEL));

    let list = List::new(items).block(block);
    let mut state = app.menu_state.clone();
    f.render_stateful_widget(list, area, &mut state);
}

fn render_main(f: &mut Frame, app: &App, area: Rect) {
    match app.selected_section {
        Section::Status => render_status(f, app, area),
        Section::MintBurn => render_mint_burn(f, app, area),
        Section::FreezeThaw => render_freeze_thaw(f, app, area),
        Section::Blacklist => render_blacklist(f, app, area),
        Section::Minters => render_minters(f, app, area),
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

fn render_status(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(10)])
        .split(area);

    // Main status
    let content: Vec<Line> = if app.status_lines.is_empty() {
        vec![
            Line::from(""),
            Line::from(vec![Span::styled("  Press R to fetch status", Style::default().fg(DIM))]),
        ]
    } else {
        app.status_lines.iter().map(|l| {
            let parts: Vec<&str> = l.splitn(2, ':').collect();
            if parts.len() == 2 {
                Line::from(vec![
                    Span::styled(format!("  {:<10}", parts[0]), Style::default().fg(DIM)),
                    Span::styled(": ", Style::default().fg(DIM)),
                    Span::styled(parts[1].trim().to_string(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
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

    // Messages log
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
        Line::from(vec![Span::styled("  ────────────────────────────────────────", Style::default().fg(DIM))]),
        Line::from(vec![
            Span::styled("  [M]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("  Mint tokens to a recipient address", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled("  BURN", Style::default().fg(ORANGE).add_modifier(Modifier::BOLD))]),
        Line::from(vec![Span::styled("  ────────────────────────────────────────", Style::default().fg(DIM))]),
        Line::from(vec![
            Span::styled("  [B]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("  Burn tokens from your account", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled("  Press the shortcut key to open the action form.", Style::default().fg(DIM))]),
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
        Line::from(vec![Span::styled("  FREEZE ACCOUNT", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))]),
        Line::from(vec![Span::styled("  ────────────────────────────────────────", Style::default().fg(DIM))]),
        Line::from(vec![
            Span::styled("  [F]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("  Freeze a wallet's token account", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled("  THAW ACCOUNT", Style::default().fg(GREEN).add_modifier(Modifier::BOLD))]),
        Line::from(vec![Span::styled("  ────────────────────────────────────────", Style::default().fg(DIM))]),
        Line::from(vec![
            Span::styled("  [T]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("  Thaw a previously frozen account", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled("  Press the shortcut key to open the action form.", Style::default().fg(DIM))]),
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

    // Actions
    let actions = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  [A]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("  Add address to blacklist", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [R]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("  Remove from blacklist", Style::default().fg(Color::White)),
        ]),
    ];

    let para = Paragraph::new(actions)
        .block(panel_block("ACTIONS", RED))
        .wrap(Wrap { trim: false });
    f.render_widget(para, main_chunks[0]);

    // List
    let items: Vec<ListItem> = if app.blacklist_entries.is_empty() {
        vec![ListItem::new(Line::from(vec![Span::styled("  No entries", Style::default().fg(DIM))]))]
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

    // Actions
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

    // List
    let items: Vec<ListItem> = if app.minter_entries.is_empty() {
        vec![ListItem::new(Line::from(vec![Span::styled("  No minters added", Style::default().fg(DIM))]))]
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

fn render_messages(f: &mut Frame, app: &App, area: Rect) {
    let content: Vec<Line> = if app.messages.is_empty() {
        vec![Line::from(vec![Span::styled("  No recent activity", Style::default().fg(DIM))])]
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
        Section::Status => " [R] Refresh   [↑↓] Navigate   [Q] Quit",
        Section::MintBurn => " [M] Mint   [B] Burn   [↑↓] Navigate   [Q] Quit",
        Section::FreezeThaw => " [F] Freeze   [T] Thaw   [↑↓] Navigate   [Q] Quit",
        Section::Blacklist => " [A] Add   [R] Remove   [↑↓] Navigate   [Q] Quit",
        Section::Minters => " [A] Add   [R] Remove   [↑↓] Navigate   [Q] Quit",
    };

    let wallet_short = if app.wallet.len() > 12 {
        format!("{}...{}", &app.wallet[..6], &app.wallet[app.wallet.len()-4..])
    } else {
        app.wallet.clone()
    };

    let line = Line::from(vec![
        Span::styled(shortcuts, Style::default().fg(DIM)),
        Span::raw("   "),
        Span::styled("Wallet: ", Style::default().fg(DIM)),
        Span::styled(wallet_short, Style::default().fg(CYAN)),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(DIM))
        .style(Style::default().bg(PANEL));

    let para = Paragraph::new(line).block(block);
    f.render_widget(para, area);
}

fn render_modal(f: &mut Frame, app: &App, area: Rect) {
    let title = match &app.modal {
        ActionModal::Mint => "MINT TOKENS",
        ActionModal::Burn => "BURN TOKENS",
        ActionModal::Freeze => "FREEZE ACCOUNT",
        ActionModal::Thaw => "THAW ACCOUNT",
        ActionModal::BlacklistAdd => "ADD TO BLACKLIST",
        ActionModal::BlacklistRemove => "REMOVE FROM BLACKLIST",
        ActionModal::AddMinter => "ADD MINTER",
        ActionModal::RemoveMinter => "REMOVE MINTER",
        _ => "",
    };

    let border_color = match &app.modal {
        ActionModal::Mint => GREEN,
        ActionModal::Burn => ORANGE,
        ActionModal::Freeze => CYAN,
        ActionModal::Thaw => GREEN,
        ActionModal::BlacklistAdd | ActionModal::BlacklistRemove => RED,
        ActionModal::AddMinter | ActionModal::RemoveMinter => GREEN,
        _ => CYAN,
    };

    let height = 6 + (app.fields.len() as u16 * 3);
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

    // Inner content
    let inner = Rect {
        x: modal_area.x + 2,
        y: modal_area.y + 2,
        width: modal_area.width.saturating_sub(4),
        height: modal_area.height.saturating_sub(4),
    };

    let mut y_offset = inner.y;

    for (i, field) in app.fields.iter().enumerate() {
        let focused = app.focused_field == i;
        let label_area = Rect { x: inner.x, y: y_offset, width: inner.width, height: 1 };
        let input_area = Rect { x: inner.x, y: y_offset + 1, width: inner.width, height: 2 };

        // Label
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    field.label.clone(),
                    Style::default().fg(if focused { border_color } else { DIM })
                        .add_modifier(Modifier::BOLD),
                ),
            ])),
            label_area,
        );

        // Input value or placeholder
        let display = if field.value.is_empty() {
            Span::styled(field.placeholder.clone(), Style::default().fg(DIM))
        } else {
            Span::styled(
                format!("{}{}", field.value, if focused { "█" } else { "" }),
                Style::default().fg(Color::White),
            )
        };

        let input_block = Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(if focused { border_color } else { DIM }));

        let input_para = Paragraph::new(Line::from(vec![display])).block(input_block);
        f.render_widget(input_para, input_area);

        y_offset += 4;
    }

    // Hint
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

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(CYAN))
        .style(Style::default().bg(PANEL));

    let para = Paragraph::new(Line::from(vec![
        Span::styled(format!(" {} ", spinner), Style::default().fg(CYAN)),
        Span::styled(app.loading_text.clone(), Style::default().fg(Color::White)),
    ]))
    .block(block)
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
