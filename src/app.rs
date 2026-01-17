use std::io::{self, Stdout, Write};
use std::sync::Arc;
use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use eyre::Result;
use ratatui::{Terminal, prelude::*};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::config::AppConfig;

/// Helper to temporarily restore terminal for dialoguer prompts
fn with_restored_terminal<F, T>(f: F) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    // Leave alternate screen and disable raw mode
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

    // Clear screen for clean dialog display
    print!("\x1B[2J\x1B[1;1H");
    io::stdout().flush()?;

    // Run the closure
    let result = f();

    // Restore TUI mode
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;

    result
}
use crate::contracts::DeploymentManager;
use crate::project::Project;
use crate::scripts::ScriptManager;
use crate::ui;

/// Application state
pub struct App {
    pub project: Project,
    pub config: AppConfig,
    pub view: View,
    pub should_quit: bool,
    pub deployments: DeploymentManager,
    pub scripts: Arc<ScriptManager>,
    pub status_message: Option<String>,
    pub script_tx: UnboundedSender<Action>,
}

pub enum Action {
    ScriptLine(String),
    ScriptFinished(Result<String>),
}

/// Current view/screen
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Home,
    Interact(InteractState),
    Scripts(ScriptsState),
    Config,
}

/// Which panel is focused in interact view
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum InteractFocus {
    #[default]
    Deployments,
    Functions,
    Inputs,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InteractState {
    pub focus: InteractFocus,
    pub selected_deployment: usize,
    pub selected_function: usize,
    pub input_values: Vec<String>,
    pub current_input: usize,
    pub result: Option<String>,
    pub error: Option<String>,
}

/// Phase of script execution flow
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ScriptPhase {
    #[default]
    SelectScript,
    SelectNetwork { selected: usize },
    SelectWallet { network_idx: usize, selected: usize },
    Running,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScriptsState {
    pub selected_script: usize,
    pub phase: ScriptPhase,
    pub output: Option<String>,
}

impl App {
    pub fn new(project: Project, script_tx: UnboundedSender<Action>) -> Result<Self> {
        let config = AppConfig::load()?;
        let deployments = DeploymentManager::new(&project);
        let scripts = Arc::new(ScriptManager::new(&project));

        Ok(Self {
            project,
            config,
            view: View::Home,
            should_quit: false,
            deployments,
            scripts,
            status_message: None,
            script_tx,
        })
    }

    #[allow(dead_code)]
    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some(msg.into());
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
    }

    /// Get the default network configuration
    pub fn get_default_network(&self) -> Option<(&String, &crate::config::NetworkConfig)> {
        self.config.get_network(None)
    }
}

/// Main entry point for running the TUI application
pub async fn run(project: Project) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create channel for script actions
    let (tx, rx) = mpsc::unbounded_channel::<Action>();

    // Create app state
    let mut app = App::new(project, tx)?;

    // Scan for deployments and scripts
    app.deployments.scan()?;
    Arc::get_mut(&mut app.scripts).unwrap().scan()?;

    // Run main loop
    let result = run_app(&mut terminal, &mut app, rx).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    mut rx: UnboundedReceiver<Action>
) -> Result<()> {
    loop {
        // Handle script actions
        while let Ok(action) = rx.try_recv() {
            match action {
                Action::ScriptLine(line) => {
                    if let View::Scripts(state) = &mut app.view {
                        if let Some(output) = &mut state.output {
                            output.push_str(&line);
                            output.push('\n');
                        } else {
                            state.output = Some(format!("{}\n", line));
                        }
                    }
                }
                Action::ScriptFinished(result) => {
                    if let View::Scripts(state) = &mut app.view {
                        // Keep in Running phase so output remains visible
                        // User presses Esc to return to SelectScript
                        match result {
                            Ok(_stdout) => {
                                state.output = Some(format!(
                                    "{}\n─── Finished ───\nPress Esc to continue",
                                    state.output.as_deref().unwrap_or("")
                                ));
                            }
                            Err(e) => {
                                state.output = Some(format!(
                                    "{}\n─── Error ───\n{}\n\nPress Esc to continue",
                                    state.output.as_deref().unwrap_or(""),
                                    e
                                ));
                            }
                        }
                    }
                }
            }
        }

        // Draw UI
        terminal.draw(|frame| ui::draw(frame, app))?;

        // Handle events with timeout
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                // Global quit: Ctrl+C or q from home
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    app.should_quit = true;
                }

                // Handle input based on current view
                match &app.view {
                    View::Home => handle_home_input(app, key.code),
                    View::Interact(_) => handle_interact_input(app, key.code).await,
                    View::Scripts(_) => handle_scripts_input(app, key.code).await,
                    View::Config => handle_config_input(app, key.code),
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

fn handle_home_input(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('i') => {
            app.view = View::Interact(InteractState::default());
            app.clear_status();
        }
        KeyCode::Char('s') => {
            app.view = View::Scripts(ScriptsState::default());
            app.clear_status();
        }
        KeyCode::Char('c') => {
            app.view = View::Config;
            app.clear_status();
        }
        _ => {}
    }
}

async fn handle_interact_input(app: &mut App, key: KeyCode) {
    let deployments_count = app.deployments.deployments.len();

    // Get function count for selected deployment
    let functions_count = app
        .deployments
        .deployments
        .get(
            match &app.view {
                View::Interact(s) => s.selected_deployment,
                _ => 0,
            },
        )
        .map(|d| d.functions.len())
        .unwrap_or(0);

    let state = match &mut app.view {
        View::Interact(state) => state,
        _ => return,
    };

    match state.focus {
        InteractFocus::Deployments => {
            match key {
                KeyCode::Esc => app.view = View::Home,
                KeyCode::Up | KeyCode::Char('k') => {
                    state.selected_deployment = state.selected_deployment.saturating_sub(1);
                    state.selected_function = 0; // Reset function selection
                    state.result = None;
                    state.error = None;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let max = deployments_count.saturating_sub(1);
                    state.selected_deployment = (state.selected_deployment + 1).min(max);
                    state.selected_function = 0; // Reset function selection
                    state.result = None;
                    state.error = None;
                }
                KeyCode::Enter | KeyCode::Tab | KeyCode::Right => {
                    if deployments_count > 0 && functions_count > 0 {
                        state.focus = InteractFocus::Functions;
                    }
                }
                _ => {}
            }
        }

        InteractFocus::Functions => {
            match key {
                KeyCode::Esc | KeyCode::Left => {
                    state.focus = InteractFocus::Deployments;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    state.selected_function = state.selected_function.saturating_sub(1);
                    state.result = None;
                    state.error = None;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let max = functions_count.saturating_sub(1);
                    state.selected_function = (state.selected_function + 1).min(max);
                    state.result = None;
                    state.error = None;
                }
                KeyCode::Enter => {
                    // Get selected function and prepare input fields
                    if let Some(deployment) = app.deployments.deployments.get(state.selected_deployment) {
                        if let Some(func) = deployment.functions.get(state.selected_function) {
                            if func.inputs.is_empty() {
                                // No inputs needed, execute directly
                                // TODO: Execute contract call
                                state.result = Some(format!("Calling {}()...", func.name));
                            } else {
                                // Prepare input fields
                                state.input_values = vec![String::new(); func.inputs.len()];
                                state.current_input = 0;
                                state.focus = InteractFocus::Inputs;
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        InteractFocus::Inputs => {
            match key {
                KeyCode::Esc => {
                    state.focus = InteractFocus::Functions;
                    state.input_values.clear();
                }
                KeyCode::Enter => {
                    // Move to next input or submit
                    if state.current_input + 1 < state.input_values.len() {
                        state.current_input += 1;
                    } else {
                        // Execute the call
                        if let Some(deployment) = app.deployments.deployments.get(state.selected_deployment) {
                            if let Some(func) = deployment.functions.get(state.selected_function) {
                                let params: Vec<String> = state.input_values.clone();
                                state.result = Some(format!(
                                    "Calling {}({})...",
                                    func.name,
                                    params.join(", ")
                                ));
                                // TODO: Actually execute the contract call here
                            }
                        }
                        state.focus = InteractFocus::Functions;
                        state.input_values.clear();
                    }
                }
                KeyCode::Tab => {
                    if state.input_values.len() > 1 {
                        state.current_input = (state.current_input + 1) % state.input_values.len();
                    }
                }
                KeyCode::BackTab => {
                    if state.input_values.len() > 1 {
                        if state.current_input > 0 {
                            state.current_input -= 1;
                        } else {
                            state.current_input = state.input_values.len() - 1;
                        }
                    }
                }
                KeyCode::Up => {
                    if state.current_input > 0 {
                        state.current_input -= 1;
                    }
                }
                KeyCode::Down => {
                    if state.current_input + 1 < state.input_values.len() {
                        state.current_input += 1;
                    }
                }
                KeyCode::Backspace => {
                    if let Some(input) = state.input_values.get_mut(state.current_input) {
                        input.pop();
                    }
                }
                KeyCode::Char(c) => {
                    if let Some(input) = state.input_values.get_mut(state.current_input) {
                        input.push(c);
                    }
                }
                _ => {}
            }
        }
    }
}

async fn handle_scripts_input(app: &mut App, key: KeyCode) {
    let scripts_manager = app.scripts.clone();
    let scripts = &scripts_manager.scripts;
    let network_count = app.config.networks.len();
    let wallet_count = app.config.wallets.len() + 1; // +1 for "Use env var" option

    // Get current phase
    let phase = match &app.view {
        View::Scripts(state) => state.phase.clone(),
        _ => return,
    };

    match phase {
        ScriptPhase::SelectScript => {
            let selected_script = match &app.view {
                View::Scripts(state) => state.selected_script,
                _ => return,
            };

            match key {
                KeyCode::Esc => app.view = View::Home,
                KeyCode::Up | KeyCode::Char('k') => {
                    if let View::Scripts(state) = &mut app.view {
                        state.selected_script = selected_script.saturating_sub(1);
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let max = scripts.len().saturating_sub(1);
                    if let View::Scripts(state) = &mut app.view {
                        state.selected_script = (selected_script + 1).min(max);
                    }
                }
                KeyCode::Enter => {
                    if !scripts.is_empty() && network_count > 0 {
                        // Find default network index
                        let default_idx = app
                            .config
                            .defaults
                            .as_ref()
                            .and_then(|d| d.network.as_ref())
                            .and_then(|default| {
                                app.config.networks.keys().position(|n| n == default)
                            })
                            .unwrap_or(0);

                        if let View::Scripts(state) = &mut app.view {
                            state.phase = ScriptPhase::SelectNetwork { selected: default_idx };
                            state.output = Some("Select network (↑↓ to navigate, Enter to confirm, Esc to cancel)".to_string());
                        }
                    } else if network_count == 0 {
                        app.set_status("No networks configured. Add networks in config first.");
                    }
                }
                _ => {}
            }
        }

        ScriptPhase::SelectNetwork { selected } => {
            match key {
                KeyCode::Esc => {
                    if let View::Scripts(state) = &mut app.view {
                        state.phase = ScriptPhase::SelectScript;
                        state.output = None;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if let View::Scripts(state) = &mut app.view {
                        state.phase = ScriptPhase::SelectNetwork {
                            selected: selected.saturating_sub(1),
                        };
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let max = network_count.saturating_sub(1);
                    if let View::Scripts(state) = &mut app.view {
                        state.phase = ScriptPhase::SelectNetwork {
                            selected: (selected + 1).min(max),
                        };
                    }
                }
                KeyCode::Enter => {
                    // Find default wallet index
                    let wallet_names: Vec<String> = app.config.wallets.keys().cloned().collect();
                    let default_wallet_idx = app
                        .config
                        .defaults
                        .as_ref()
                        .and_then(|d| d.wallet.as_ref())
                        .and_then(|default| wallet_names.iter().position(|w| w == default))
                        .map(|idx| idx + 1) // +1 because env var is at index 0
                        .unwrap_or(0);

                    if let View::Scripts(state) = &mut app.view {
                        state.phase = ScriptPhase::SelectWallet {
                            network_idx: selected,
                            selected: default_wallet_idx,
                        };
                        state.output = Some("Select wallet (↑↓ to navigate, Enter to run, Esc to go back)".to_string());
                    }
                }
                _ => {}
            }
        }

        ScriptPhase::SelectWallet { network_idx, selected } => {
            match key {
                KeyCode::Esc => {
                    if let View::Scripts(state) = &mut app.view {
                        state.phase = ScriptPhase::SelectNetwork { selected: network_idx };
                        state.output = Some("Select network (↑↓ to navigate, Enter to confirm, Esc to cancel)".to_string());
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if let View::Scripts(state) = &mut app.view {
                        state.phase = ScriptPhase::SelectWallet {
                            network_idx,
                            selected: selected.saturating_sub(1),
                        };
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let max = wallet_count.saturating_sub(1);
                    if let View::Scripts(state) = &mut app.view {
                        state.phase = ScriptPhase::SelectWallet {
                            network_idx,
                            selected: (selected + 1).min(max),
                        };
                    }
                }
                KeyCode::Enter => {
                    // Execute the script
                    let selected_script = match &app.view {
                        View::Scripts(state) => state.selected_script,
                        _ => return,
                    };

                    if let Some(script) = scripts.get(selected_script) {
                        let script_clone = script.clone();
                        let config_clone = app.config.clone();
                        let tx = app.script_tx.clone();

                        // Get network name
                        let network_names: Vec<String> = app.config.networks.keys().cloned().collect();
                        let network_name = network_names.get(network_idx).cloned().unwrap_or_default();

                        // Get wallet name (None = use env var)
                        let wallet_names: Vec<String> = app.config.wallets.keys().cloned().collect();
                        let wallet_name = if selected == 0 {
                            None
                        } else {
                            wallet_names.get(selected - 1).cloned()
                        };

                        if let View::Scripts(state) = &mut app.view {
                            state.phase = ScriptPhase::Running;
                            state.output = Some(format!(
                                "Running {} on {} with wallet {}...\n\n",
                                script_clone.name,
                                network_name,
                                wallet_name.as_deref().unwrap_or("(env)")
                            ));
                        }

                        // Spawn script execution
                        tokio::spawn(async move {
                            let (line_tx, mut line_rx) = mpsc::unbounded_channel::<String>();
                            let tx_for_run = Some(line_tx);

                            let tx_clone = tx.clone();
                            tokio::spawn(async move {
                                while let Some(line) = line_rx.recv().await {
                                    let _ = tx_clone.send(Action::ScriptLine(line));
                                }
                            });

                            let result = scripts_manager
                                .run_with_wallet(
                                    &script_clone,
                                    &network_name,
                                    wallet_name.as_deref(),
                                    &config_clone,
                                    true,
                                    false,
                                    tx_for_run,
                                )
                                .await;

                            match result {
                                Ok(output) => {
                                    let _ = tx.send(Action::ScriptFinished(Ok(output.stdout)));
                                }
                                Err(e) => {
                                    let _ = tx.send(Action::ScriptFinished(Err(e)));
                                }
                            }
                        });
                    }
                }
                _ => {}
            }
        }

        ScriptPhase::Running => {
            if key == KeyCode::Esc {
                if let View::Scripts(state) = &mut app.view {
                    state.phase = ScriptPhase::SelectScript;
                    // Keep output visible
                }
            }
        }
    }
}

fn handle_config_input(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Esc | KeyCode::Char('q') => app.view = View::Home,
        KeyCode::Char('k') => {
            if let Err(e) = handle_add_wallet(app) {
                app.set_status(format!("Failed to add wallet: {}", e));
            } else {
                app.set_status("Wallet added successfully");
            }
        }
        KeyCode::Char('r') => {
            if let Err(e) = handle_add_rpc_url(app) {
                app.set_status(format!("Failed to add RPC URL: {}", e));
            } else {
                app.set_status("RPC URL added successfully");
            }
        }
        KeyCode::Char('a') => {
            if let Err(e) = handle_add_api_key(app) {
                app.set_status(format!("Failed to add API key: {}", e));
            } else {
                app.set_status("API key added successfully");
            }
        }
        KeyCode::Char('d') => {
            if let Err(e) = handle_delete_credentials(app) {
                app.set_status(format!("Failed to delete credentials: {}", e));
            } else {
                app.set_status("Credentials deleted successfully");
            }
        }
        KeyCode::Char('e') => {
            if let Err(e) = handle_export_private_key(app) {
                app.set_status(format!("Export failed: {}", e));
            }
        }
        _ => {}
    }
}

fn handle_export_private_key(app: &mut App) -> Result<()> {
    use crate::config::get_private_key;
    use dialoguer::{Confirm, Select};

    let wallet_names: Vec<String> = app.config.wallets.keys().cloned().collect();
    if wallet_names.is_empty() {
        app.set_status("No wallets to export");
        return Ok(());
    }

    // Clone wallet data needed for the dialog
    let wallets_data: Vec<(String, Option<String>, Option<String>)> = wallet_names
        .iter()
        .map(|name| {
            let wallet = app.config.wallets.get(name).unwrap();
            (
                name.clone(),
                wallet.keychain.clone(),
                wallet.env_var.clone(),
            )
        })
        .collect();

    let status_msg = with_restored_terminal(|| {
        let selection = Select::new()
            .with_prompt("Select wallet to export")
            .items(&wallet_names)
            .interact()?;

        let (wallet_name, keychain_ref, env_var) = &wallets_data[selection];

        if let Some(keychain_ref) = keychain_ref {
            let key_name = keychain_ref
                .trim()
                .strip_prefix("runic:")
                .unwrap_or(keychain_ref.trim());

            if let Some(pk) = get_private_key(key_name)? {
                println!("\nWallet: {}", wallet_name);
                println!("Private Key: {}", *pk);
                println!("\nSECURITY WARNING: Keep this key secret! Never share it.");

                Confirm::new()
                    .with_prompt("Press Enter to clear screen and continue")
                    .default(true)
                    .show_default(false)
                    .interact()?;

                Ok(None)
            } else {
                Ok(Some(format!(
                    "Private key not found in keychain for '{}'. Try re-adding the wallet with 'k'.",
                    wallet_name
                )))
            }
        } else if let Some(env_var) = env_var {
            match std::env::var(env_var) {
                Ok(pk) => {
                    println!("\nWallet: {} (from env: {})", wallet_name, env_var);
                    println!("Private Key: {}", pk);
                    println!("\nSECURITY WARNING: Keep this key secret! Never share it.");

                    Confirm::new()
                        .with_prompt("Press Enter to clear screen and continue")
                        .default(true)
                        .show_default(false)
                        .interact()?;

                    Ok(None)
                }
                Err(_) => Ok(Some(format!(
                    "Environment variable '{}' not set for wallet '{}'",
                    env_var, wallet_name
                ))),
            }
        } else {
            Ok(Some("Wallet has no keychain or env_var configured".to_string()))
        }
    })?;

    if let Some(msg) = status_msg {
        app.set_status(msg);
    }

    Ok(())
}

fn handle_add_wallet(app: &mut App) -> Result<()> {
    use crate::config::{Defaults, WalletConfig, store_private_key, get_private_key};
    use dialoguer::{Confirm, Input, Password};

    let (wallet_name, private_key, label, set_default) = with_restored_terminal(|| {
        let wallet_name_input: String = Input::new()
            .with_prompt("Enter wallet name")
            .interact()?;
        let wallet_name = wallet_name_input.trim().to_string();

        if wallet_name.is_empty() {
            return Err(eyre::eyre!("Wallet name cannot be empty"));
        }

        let private_key = Password::new()
            .with_prompt("Enter private key (64 hex chars, with or without 0x)")
            .interact()?;

        if private_key.is_empty() {
            return Err(eyre::eyre!("Private key cannot be empty"));
        }

        // Validate key format before confirmation
        let clean_key = private_key.trim().strip_prefix("0x").unwrap_or(private_key.trim());
        if clean_key.len() != 64 || !clean_key.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(eyre::eyre!(
                "Invalid private key format: expected 64 hex characters (got {} chars)",
                clean_key.len()
            ));
        }

        let label: Option<String> = Input::<String>::new()
            .with_prompt("Enter wallet label (optional)")
            .allow_empty(true)
            .interact()
            .ok()
            .filter(|s| !s.is_empty());

        let set_default = Confirm::new()
            .with_prompt("Set as default wallet for script execution?")
            .default(true)
            .interact()?;

        Ok((wallet_name, private_key, label, set_default))
    })?;

    // Store in keychain
    store_private_key(&wallet_name, &private_key)?;

    // Verify it was stored correctly
    match get_private_key(&wallet_name)? {
        Some(_) => {
            tracing::info!("Private key stored and verified for wallet: {}", wallet_name);
        }
        None => {
            return Err(eyre::eyre!(
                "Failed to verify private key storage. The keychain may have denied access."
            ));
        }
    }

    app.config.wallets.insert(
        wallet_name.clone(),
        WalletConfig {
            keychain: Some(format!("runic:{}", wallet_name)),
            env_var: None,
            label,
        },
    );

    if set_default {
        if let Some(defaults) = &mut app.config.defaults {
            defaults.wallet = Some(wallet_name.clone());
        } else {
            app.config.defaults = Some(Defaults {
                network: None,
                wallet: Some(wallet_name.clone()),
            });
        }
    }

    app.config.save()?;

    Ok(())
}

enum DeleteAction {
    Wallet(String),
    Network(String),
    ApiKey(String),
    None,
}

fn handle_delete_credentials(app: &mut App) -> Result<()> {
    use crate::config::KeychainManager;
    use dialoguer::{Confirm, Select};

    let wallet_names: Vec<String> = app.config.wallets.keys().cloned().collect();
    let network_names: Vec<String> = app.config.networks.keys().cloned().collect();
    let api_names: Vec<String> = app.config.api_keys.keys().cloned().collect();

    let action = with_restored_terminal(|| {
        let options = vec!["Delete private key", "Delete RPC URL", "Delete API key"];

        let selection = Select::new()
            .with_prompt("Select credential type to delete")
            .items(&options)
            .interact()?;

        match selection {
            0 => {
                if wallet_names.is_empty() {
                    println!("No wallets to delete");
                    return Ok(DeleteAction::None);
                }

                let wallet_selection = Select::new()
                    .with_prompt("Select wallet to delete")
                    .items(&wallet_names)
                    .interact()?;
                let wallet_name = wallet_names[wallet_selection].clone();

                if Confirm::new()
                    .with_prompt(&format!(
                        "Delete wallet '{}' and its private key?",
                        wallet_name
                    ))
                    .default(false)
                    .interact()?
                {
                    Ok(DeleteAction::Wallet(wallet_name))
                } else {
                    Ok(DeleteAction::None)
                }
            }
            1 => {
                if network_names.is_empty() {
                    println!("No networks to delete");
                    return Ok(DeleteAction::None);
                }

                let network_selection = Select::new()
                    .with_prompt("Select network to delete")
                    .items(&network_names)
                    .interact()?;
                let network_name = network_names[network_selection].clone();

                if Confirm::new()
                    .with_prompt(&format!(
                        "Delete network '{}' and its RPC URL?",
                        network_name
                    ))
                    .default(false)
                    .interact()?
                {
                    Ok(DeleteAction::Network(network_name))
                } else {
                    Ok(DeleteAction::None)
                }
            }
            2 => {
                if api_names.is_empty() {
                    println!("No API keys to delete");
                    return Ok(DeleteAction::None);
                }

                let service_selection = Select::new()
                    .with_prompt("Select API key to delete")
                    .items(&api_names)
                    .interact()?;
                let service_name = api_names[service_selection].clone();

                if Confirm::new()
                    .with_prompt(&format!("Delete API key for '{}'?", service_name))
                    .default(false)
                    .interact()?
                {
                    Ok(DeleteAction::ApiKey(service_name))
                } else {
                    Ok(DeleteAction::None)
                }
            }
            _ => Ok(DeleteAction::None),
        }
    })?;

    // Perform the action after terminal is restored
    match action {
        DeleteAction::Wallet(wallet_name) => {
            let km = KeychainManager::new();
            km.delete(&wallet_name)?;
            app.config.wallets.remove(&wallet_name);
            app.config.save()?;
        }
        DeleteAction::Network(network_name) => {
            let km = KeychainManager::new();
            km.delete(&format!("rpc:{}", network_name))?;
            app.config.networks.remove(&network_name);
            app.config.save()?;
        }
        DeleteAction::ApiKey(service_name) => {
            let km = KeychainManager::new();
            km.delete(&format!("api:{}", service_name))?;
            app.config.api_keys.remove(&service_name);
            app.config.save()?;
        }
        DeleteAction::None => {}
    }

    Ok(())
}

fn handle_add_rpc_url(app: &mut App) -> Result<()> {
    use crate::config::store_rpc_url;
    use dialoguer::Input;

    let (rpc_name, rpc_url, chain_id) = with_restored_terminal(|| {
        let rpc_name = Input::<String>::new()
            .with_prompt("Enter RPC URL name")
            .interact()?;

        let rpc_url = Input::<String>::new()
            .with_prompt("Enter RPC URL (e.g., https://eth.llamarpc.com)")
            .validate_with(|input: &String| {
                if input.starts_with("http://") || input.starts_with("https://") {
                    Ok(())
                } else {
                    Err("URL must start with http:// or https://".to_string())
                }
            })
            .interact()?;

        let chain_id: Option<u64> = Input::<String>::new()
            .with_prompt("Enter chain ID (optional)")
            .allow_empty(true)
            .interact()
            .ok()
            .and_then(|s| s.parse().ok());

        Ok((rpc_name, rpc_url, chain_id))
    })?;

    store_rpc_url(&rpc_name, &rpc_url)?;

    app.config.networks.insert(
        rpc_name.clone(),
        crate::config::NetworkConfig {
            rpc_url: format!("keychain:{}", rpc_name),
            chain_id,
            explorer_url: None,
            explorer_api_key: None,
        },
    );
    app.config.save()?;

    Ok(())
}

fn handle_add_api_key(app: &mut App) -> Result<()> {
    use crate::config::store_api_key;
    use dialoguer::{Input, Password};

    let (service_name, api_key) = with_restored_terminal(|| {
        let service_name = Input::<String>::new()
            .with_prompt("Enter API service name (e.g., etherscan_mainnet)")
            .interact()?;

        let api_key = Password::new()
            .with_prompt("Enter API key")
            .with_confirmation("Confirm API key", "API keys do not match")
            .interact()?;

        Ok((service_name, api_key))
    })?;

    store_api_key(&service_name, &api_key)?;

    app.config.api_keys.insert(
        service_name.clone(),
        format!("keychain:api:{}", service_name),
    );
    app.config.save()?;

    Ok(())
}