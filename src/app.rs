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
use crate::contracts::{CallResult, ContractCaller, chain_id_to_network};

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
    WalletSelection,
    AbiSelection,
    ImplementationPrompt, // Prompt for proxy implementation ABI
}

/// Status of a contract call
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CallStatus {
    /// Not in a call
    #[default]
    Idle,
    /// Connecting to RPC
    Connecting,
    /// Executing the call
    Executing,
    /// Call completed successfully
    Completed,
    /// Call failed
    Failed(String),
}

/// Network information for the call
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkInfo {
    pub network_name: String,
    pub chain_id: u64,
    pub rpc_url: String,
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
    pub call_status: CallStatus,
    pub network_info: Option<NetworkInfo>,
    pub selected_wallet: Option<String>,
    pub abi_selection_index: usize,
    pub selecting_abi_for: Option<usize>,
}

/// Phase of script execution flow
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ScriptPhase {
    #[default]
    SelectScript,
    SelectNetwork {
        selected: usize,
    },
    SelectWallet {
        network_idx: usize,
        selected: usize,
    },
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
    let missing_chain_ids = app.deployments.scan()?;
    Arc::get_mut(&mut app.scripts).unwrap().scan()?;

    // Check for missing network configurations
    if !missing_chain_ids.is_empty() {
        handle_missing_networks(&mut app, &missing_chain_ids).await?;
    }

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
    mut rx: UnboundedReceiver<Action>,
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

    let (selected_deployment_idx, selected_function_idx) = match &app.view {
        View::Interact(s) => (s.selected_deployment, s.selected_function),
        _ => return,
    };

    let (focus, input_values, current_input, selected_wallet) = match &app.view {
        View::Interact(s) => (
            s.focus.clone(),
            s.input_values.clone(),
            s.current_input,
            s.selected_wallet.clone(),
        ),
        _ => return,
    };

    let functions_count = app
        .deployments
        .deployments
        .get(selected_deployment_idx)
        .map(|d| d.functions.len())
        .unwrap_or(0);

    let deployment_clone = app
        .deployments
        .deployments
        .get(selected_deployment_idx)
        .cloned();

    match focus {
        InteractFocus::Deployments => match key {
            KeyCode::Esc => app.view = View::Home,
            KeyCode::Up | KeyCode::Char('k') => {
                if let View::Interact(state) = &mut app.view {
                    state.selected_deployment = state.selected_deployment.saturating_sub(1);
                    state.selected_function = 0;
                    state.result = None;
                    state.error = None;
                    state.network_info = None;
                    state.call_status = CallStatus::Idle;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let View::Interact(state) = &mut app.view {
                    let max = deployments_count.saturating_sub(1);
                    state.selected_deployment = (state.selected_deployment + 1).min(max);
                    state.selected_function = 0;
                    state.result = None;
                    state.error = None;
                    state.network_info = None;
                    state.call_status = CallStatus::Idle;
                }
            }
            KeyCode::Enter | KeyCode::Tab | KeyCode::Right => {
                if deployments_count > 0 {
                    // Check if this is a proxy that needs implementation confirmation
                    // A contract is considered a proxy if callable_address != address
                    if let Some(deployment) = deployment_clone.as_ref() {
                        let is_behind_proxy = deployment.callable_address != deployment.address;
                        if is_behind_proxy && !deployment.implementation_set {
                            if let View::Interact(state) = &mut app.view {
                                state.focus = InteractFocus::ImplementationPrompt;
                                state.abi_selection_index = state.selected_deployment;
                            }
                            return;
                        }
                    }
                    // Normal flow: go to functions if available
                    if functions_count > 0 {
                        if let View::Interact(state) = &mut app.view {
                            state.focus = InteractFocus::Functions;
                        }
                    }
                }
            }
            KeyCode::Char('a') => {
                 if let View::Interact(state) = &mut app.view {
                     state.focus = InteractFocus::AbiSelection;
                     state.abi_selection_index = state.selected_deployment;
                     state.selecting_abi_for = Some(state.selected_deployment);
                 }
            }
            _ => {}
        },

        InteractFocus::AbiSelection => match key {
            KeyCode::Esc => {
                 if let View::Interact(state) = &mut app.view {
                     state.focus = InteractFocus::Deployments;
                     state.selecting_abi_for = None;
                 }
            }
             KeyCode::Up | KeyCode::Char('k') => {
                if let View::Interact(state) = &mut app.view {
                    state.abi_selection_index = state.abi_selection_index.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let View::Interact(state) = &mut app.view {
                    let max = deployments_count.saturating_sub(1);
                    state.abi_selection_index = (state.abi_selection_index + 1).min(max);
                }
            }
            KeyCode::Enter => {
                let abi_idx = match &app.view {
                    View::Interact(s) => s.abi_selection_index,
                    _ => return,
                };
                let target_idx = match &app.view {
                     View::Interact(s) => s.selecting_abi_for,
                     _ => return,
                };

                if let Some(target_idx) = target_idx {
                    // Clone ABI info from source
                     if let Some(source) = app.deployments.deployments.get(abi_idx).cloned() {
                          if let Some(target) = app.deployments.deployments.get_mut(target_idx) {
                               target.functions = source.functions;
                               target.abi_path = source.abi_path;
                          }
                     }
                }

                if let View::Interact(state) = &mut app.view {
                     state.focus = InteractFocus::Deployments;
                     state.selecting_abi_for = None;
                     state.selected_function = 0; // Reset function selection
                }
            }
            _ => {}
        },

        InteractFocus::ImplementationPrompt => match key {
            KeyCode::Esc => {
                // Cancel and go back to deployments
                if let View::Interact(state) = &mut app.view {
                    state.focus = InteractFocus::Deployments;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let View::Interact(state) = &mut app.view {
                    state.abi_selection_index = state.abi_selection_index.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let View::Interact(state) = &mut app.view {
                    let max = deployments_count.saturating_sub(1);
                    state.abi_selection_index = (state.abi_selection_index + 1).min(max);
                }
            }
            KeyCode::Enter => {
                // Select the implementation ABI and mark as set
                let abi_idx = match &app.view {
                    View::Interact(s) => s.abi_selection_index,
                    _ => return,
                };

                // Clone ABI info from source to current deployment
                if let Some(source) = app.deployments.deployments.get(abi_idx).cloned() {
                    if let Some(target) = app
                        .deployments
                        .deployments
                        .get_mut(selected_deployment_idx)
                    {
                        target.functions = source.functions;
                        target.abi_path = source.abi_path;
                        target.implementation_set = true;
                    }
                }

                // Go to functions
                let new_functions_count = app
                    .deployments
                    .deployments
                    .get(selected_deployment_idx)
                    .map(|d| d.functions.len())
                    .unwrap_or(0);

                if let View::Interact(state) = &mut app.view {
                    state.selected_function = 0;
                    if new_functions_count > 0 {
                        state.focus = InteractFocus::Functions;
                    } else {
                        state.focus = InteractFocus::Deployments;
                    }
                }
            }
            KeyCode::Char('s') => {
                // Skip - use current ABI as-is
                if let Some(target) = app
                    .deployments
                    .deployments
                    .get_mut(selected_deployment_idx)
                {
                    target.implementation_set = true;
                }

                if functions_count > 0 {
                    if let View::Interact(state) = &mut app.view {
                        state.focus = InteractFocus::Functions;
                    }
                } else if let View::Interact(state) = &mut app.view {
                    state.focus = InteractFocus::Deployments;
                }
            }
            _ => {}
        },

        InteractFocus::Functions => match key {
            KeyCode::Esc | KeyCode::Left => {
                if let View::Interact(state) = &mut app.view {
                    state.focus = InteractFocus::Deployments;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let View::Interact(state) = &mut app.view {
                    state.selected_function = state.selected_function.saturating_sub(1);
                    state.result = None;
                    state.error = None;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let View::Interact(state) = &mut app.view {
                    let max = functions_count.saturating_sub(1);
                    state.selected_function = (state.selected_function + 1).min(max);
                    state.result = None;
                    state.error = None;
                }
            }
            KeyCode::Enter => {
                if let Some(deployment) = deployment_clone.as_ref() {
                    if let Some(func) = deployment.functions.get(selected_function_idx) {
                        if func.inputs.is_empty() {
                            let is_write = !ContractCaller::is_read_only(func);
                            if is_write {
                                if let View::Interact(state) = &mut app.view {
                                    state.selected_wallet =
                                        app.config.defaults.as_ref().and_then(|d| d.wallet.clone());
                                    state.focus = InteractFocus::WalletSelection;
                                }
                            } else {
                                execute_function_call(
                                    app,
                                    selected_deployment_idx,
                                    selected_function_idx,
                                    vec![],
                                    None,
                                )
                                .await;
                            }
                        } else if let View::Interact(state) = &mut app.view {
                            state.input_values = vec![String::new(); func.inputs.len()];
                            state.current_input = 0;
                            state.focus = InteractFocus::Inputs;
                        }
                    }
                }
            }
            _ => {}
        },

        InteractFocus::Inputs => match key {
            KeyCode::Esc => {
                if let View::Interact(state) = &mut app.view {
                    state.focus = InteractFocus::Functions;
                    state.input_values.clear();
                }
            }
            KeyCode::Enter => {
                if current_input + 1 < input_values.len() {
                    if let View::Interact(state) = &mut app.view {
                        state.current_input += 1;
                    }
                } else if let Some(deployment) = deployment_clone.as_ref() {
                    if let Some(func) = deployment.functions.get(selected_function_idx) {
                        let is_write = !ContractCaller::is_read_only(func);
                        if is_write {
                            if let View::Interact(state) = &mut app.view {
                                state.selected_wallet =
                                    app.config.defaults.as_ref().and_then(|d| d.wallet.clone());
                                state.focus = InteractFocus::WalletSelection;
                            }
                        } else {
                            execute_function_call(
                                app,
                                selected_deployment_idx,
                                selected_function_idx,
                                input_values,
                                None,
                            )
                            .await;
                        }
                    }
                    if let View::Interact(state) = &mut app.view {
                        if state.focus != InteractFocus::WalletSelection {
                            state.focus = InteractFocus::Functions;
                            state.input_values.clear();
                        }
                    }
                }
            }
            KeyCode::Tab => {
                if input_values.len() > 1 {
                    if let View::Interact(state) = &mut app.view {
                        state.current_input = (state.current_input + 1) % state.input_values.len();
                    }
                }
            }
            KeyCode::BackTab => {
                if input_values.len() > 1 {
                    if let View::Interact(state) = &mut app.view {
                        if state.current_input > 0 {
                            state.current_input -= 1;
                        } else {
                            state.current_input = state.input_values.len() - 1;
                        }
                    }
                }
            }
            KeyCode::Up => {
                if current_input > 0 {
                    if let View::Interact(state) = &mut app.view {
                        state.current_input -= 1;
                    }
                }
            }
            KeyCode::Down => {
                if current_input + 1 < input_values.len() {
                    if let View::Interact(state) = &mut app.view {
                        state.current_input += 1;
                    }
                }
            }
            KeyCode::Backspace => {
                if let View::Interact(state) = &mut app.view {
                    if let Some(input) = state.input_values.get_mut(state.current_input) {
                        input.pop();
                    }
                }
            }
            KeyCode::Char(c) => {
                if let View::Interact(state) = &mut app.view {
                    if let Some(input) = state.input_values.get_mut(state.current_input) {
                        input.push(c);
                    }
                }
            }
            _ => {}
        },

        InteractFocus::WalletSelection => {
            let wallet_count = app.config.wallets.len();
            let wallet_names: Vec<String> = app.config.wallets.keys().cloned().collect();

            match key {
                KeyCode::Esc => {
                    if let View::Interact(state) = &mut app.view {
                        state.focus = InteractFocus::Functions;
                        state.selected_wallet = None;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if wallet_count > 0 {
                        if let View::Interact(state) = &mut app.view {
                            let current_idx = state
                                .selected_wallet
                                .as_ref()
                                .and_then(|w| wallet_names.iter().position(|n| n == w))
                                .unwrap_or(0);
                            let new_idx = if current_idx > 0 {
                                current_idx - 1
                            } else {
                                wallet_count - 1
                            };
                            state.selected_wallet = wallet_names.get(new_idx).cloned();
                        }
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if wallet_count > 0 {
                        if let View::Interact(state) = &mut app.view {
                            let current_idx = state
                                .selected_wallet
                                .as_ref()
                                .and_then(|w| wallet_names.iter().position(|n| n == w))
                                .unwrap_or(0);
                            let new_idx = (current_idx + 1) % wallet_count;
                            state.selected_wallet = wallet_names.get(new_idx).cloned();
                        }
                    }
                }
                KeyCode::Enter => {
                    if let Some(deployment) = deployment_clone.as_ref() {
                        if let Some(func) = deployment.functions.get(selected_function_idx) {
                            if let View::Interact(state) = &mut app.view {
                                state.focus = InteractFocus::Functions;
                            }
                            let params = if func.inputs.is_empty() {
                                vec![]
                            } else {
                                input_values.clone()
                            };
                            execute_function_call(
                                app,
                                selected_deployment_idx,
                                selected_function_idx,
                                params,
                                selected_wallet.clone(),
                            )
                            .await;
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

async fn execute_function_call(
    app: &mut App,
    deployment_idx: usize,
    function_idx: usize,
    params: Vec<String>,
    wallet_name: Option<String>,
) {
    // First, extract deployment info without holding borrow on state
    let deployment_info = app.deployments.deployments.get(deployment_idx)
        .map(|d| (d.chain_id, d.callable_address.clone(), d.functions.clone()));

    let (chain_id, callable_address, functions) = match deployment_info {
        Some(info) => info,
        None => {
            if let View::Interact(state) = &mut app.view {
                state.call_status = CallStatus::Failed("Deployment not found".to_string());
            }
            return;
        }
    };

    let func = match functions.get(function_idx) {
        Some(f) => f.clone(),
        None => {
            if let View::Interact(state) = &mut app.view {
                state.call_status = CallStatus::Failed("Function not found".to_string());
            }
            return;
        }
    };

    // Clear previous results
    if let View::Interact(state) = &mut app.view {
        state.result = None;
        state.error = None;
    }

    // Try to find the network that matches the deployment's chain ID
    // Do this outside the state borrow so we can prompt for network if needed
    let network_result = app.config.get_network_by_chain_id(chain_id);

    let (network_name, rpc_url): (String, String) = match network_result {
        Some((name, _network)) => {
            // Found network with matching chain ID - use it
            match app.config.resolve_rpc_url(name) {
                Ok(Some(url)) => (name.clone(), url),
                Ok(None) => {
                    if let View::Interact(state) = &mut app.view {
                        state.error =
                            Some(format!("No RPC URL configured for network: {}", name));
                        state.call_status = CallStatus::Failed("No RPC URL".to_string());
                    }
                    return;
                }
                Err(e) => {
                    if let View::Interact(state) = &mut app.view {
                        state.error = Some(format!("Failed to resolve RPC URL: {}", e));
                        state.call_status = CallStatus::Failed(format!("RPC error: {}", e));
                    }
                    return;
                }
            }
        }
        None => {
            // No network with matching chain ID found - prompt user to add one
            let suggested_name = crate::contracts::chain_id_to_network(chain_id);

            match prompt_add_network_for_chain(app, chain_id, &suggested_name) {
                Ok(Some(url)) => (suggested_name, url),
                Ok(None) => {
                    // User cancelled
                    if let View::Interact(state) = &mut app.view {
                        state.call_status = CallStatus::Idle;
                    }
                    return;
                }
                Err(e) => {
                    if let View::Interact(state) = &mut app.view {
                        state.error = Some(format!("Failed to add network: {}", e));
                        state.call_status = CallStatus::Failed("Config error".to_string());
                    }
                    return;
                }
            }
        }
    };

    // Update state with network info and set connecting status
    if let View::Interact(state) = &mut app.view {
        state.network_info = Some(NetworkInfo {
            network_name: network_name.clone(),
            chain_id,
            rpc_url: rpc_url.clone(),
        });
        state.call_status = CallStatus::Connecting;
    }

    let caller = ContractCaller::new(&rpc_url, chain_id);

    let result = if ContractCaller::is_read_only(&func) {
        if let View::Interact(state) = &mut app.view {
            state.call_status = CallStatus::Executing;
        }
        caller.call_read(&callable_address, &func, &params).await
    } else {
        let resolved_wallet = wallet_name
            .or_else(|| app.config.defaults.as_ref().and_then(|d| d.wallet.clone()));

        match resolved_wallet {
            Some(w_name) => match app.config.resolve_wallet_key(&w_name) {
                Ok(Some(private_key)) => {
                    if let View::Interact(state) = &mut app.view {
                        state.call_status = CallStatus::Executing;
                    }
                    match caller.with_signer(private_key) {
                        Ok(caller_with_signer) => {
                            caller_with_signer
                                .call_write(&callable_address, &func, &params, None)
                                .await
                        }
                        Err(e) => Err(eyre::eyre!("Failed to set signer: {}", e)),
                    }
                }
                Ok(None) => {
                    if let View::Interact(state) = &mut app.view {
                        state.call_status = CallStatus::Failed("Wallet not found".to_string());
                    }
                    Err(eyre::eyre!("Private key not found for wallet: {}", w_name))
                }
                Err(e) => {
                    if let View::Interact(state) = &mut app.view {
                        state.call_status = CallStatus::Failed(format!("Wallet error: {}", e));
                    }
                    Err(e)
                }
            },
            None => {
                if let View::Interact(state) = &mut app.view {
                    state.call_status = CallStatus::Failed("No wallet configured".to_string());
                }
                Err(eyre::eyre!(
                    "Write transaction requires a wallet. Configure one in settings."
                ))
            }
        }
    };

    // Update state with result
    if let View::Interact(state) = &mut app.view {
        match result {
            Ok(CallResult::Read(outputs)) => {
                state.call_status = CallStatus::Completed;
                if outputs.is_empty() {
                    state.result = Some("Call successful (no return values)".to_string());
                } else {
                    state.result = Some(format!("Result: {}", outputs.join(", ")));
                }
            }
            Ok(CallResult::Write(tx_hash)) => {
                state.call_status = CallStatus::Completed;
                state.result = Some(format!("Transaction sent: {}", tx_hash));
            }
            Ok(CallResult::Error(msg)) => {
                state.call_status = CallStatus::Failed(msg.clone());
                state.error = Some(format!("Call error: {}", msg));
            }
            Err(e) => {
                state.call_status = CallStatus::Failed(e.to_string());
                state.error = Some(format!("Call failed: {}", e));
            }
        }
    }
}

/// Prompt user to add an RPC URL for a specific chain ID
/// Returns Ok(Some(rpc_url)) if added, Ok(None) if cancelled, Err on failure
fn prompt_add_network_for_chain(
    app: &mut App,
    chain_id: u64,
    network_name: &str,
) -> Result<Option<String>> {
    use crate::config::store_rpc_url;
    use dialoguer::{Confirm, Input};

    let message = format!(
        "No RPC configured for {} (chain {}).\n\
         Would you like to add one now?",
        network_name, chain_id
    );

    let should_add = with_restored_terminal(|| {
        Confirm::new()
            .with_prompt(&message)
            .default(true)
            .interact()
            .map_err(eyre::Error::from)
    })?;

    if !should_add {
        return Ok(None);
    }

    let rpc_url: String = with_restored_terminal(|| {
        Input::<String>::new()
            .with_prompt(format!("Enter RPC URL for {} (chain {})", network_name, chain_id))
            .validate_with(|input: &String| {
                if input.starts_with("http://") || input.starts_with("https://") {
                    Ok(())
                } else {
                    Err("URL must start with http:// or https://".to_string())
                }
            })
            .interact()
            .map_err(eyre::Error::from)
    })?;

    // Store in keychain
    store_rpc_url(network_name, &rpc_url)?;

    // Add to config
    app.config.networks.insert(
        network_name.to_string(),
        crate::config::NetworkConfig {
            rpc_url: format!("keychain:{}", network_name),
            chain_id: Some(chain_id),
            explorer_url: None,
            explorer_api_key: None,
        },
    );

    app.config.save()?;

    Ok(Some(rpc_url))
}

async fn handle_missing_networks(app: &mut App, missing_chain_ids: &[u64]) -> Result<()> {
    use dialoguer::{Confirm, Input};

    for &chain_id in missing_chain_ids {
        let network_name = chain_id_to_network(chain_id);

        // Check if we already have a network with this chain_id
        let has_matching_network = app
            .config
            .networks
            .values()
            .any(|net| net.chain_id == Some(chain_id));

        if !has_matching_network {
            let message = format!(
                "Found deployment on chain {} (ID: {}) but no network configured.\n\
                 Would you like to add an RPC URL for this network?",
                network_name, chain_id
            );

            let should_add = with_restored_terminal(|| {
                Confirm::new()
                    .with_prompt(&message)
                    .default(true)
                    .interact()
                    .map_err(eyre::Error::from)
            })?;

            if should_add {
                let rpc_url: String = with_restored_terminal(|| {
                    Input::<String>::new()
                        .with_prompt(format!(
                            "Enter RPC URL for {} (chain {})",
                            network_name, chain_id
                        ))
                        .validate_with(|input: &String| {
                            if input.starts_with("http://") || input.starts_with("https://") {
                                Ok(())
                            } else {
                                Err("URL must start with http:// or https://".to_string())
                            }
                        })
                        .interact()
                        .map_err(eyre::Error::from)
                })?;

                app.config.networks.insert(
                    network_name.clone(),
                    crate::config::NetworkConfig {
                        rpc_url: format!("keychain:{}", network_name),
                        chain_id: Some(chain_id),
                        explorer_url: None,
                        explorer_api_key: None,
                    },
                );

                // Store the RPC URL in keychain
                use crate::config::store_rpc_url;
                store_rpc_url(&network_name, &rpc_url)?;

                app.config.save()?;
            }
        }
    }

    Ok(())
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
                            state.phase = ScriptPhase::SelectNetwork {
                                selected: default_idx,
                            };
                            state.output = Some(
                                "Select network (↑↓ to navigate, Enter to confirm, Esc to cancel)"
                                    .to_string(),
                            );
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
                        state.output = Some(
                            "Select wallet (↑↓ to navigate, Enter to run, Esc to go back)"
                                .to_string(),
                        );
                    }
                }
                _ => {}
            }
        }

        ScriptPhase::SelectWallet {
            network_idx,
            selected,
        } => {
            match key {
                KeyCode::Esc => {
                    if let View::Scripts(state) = &mut app.view {
                        state.phase = ScriptPhase::SelectNetwork {
                            selected: network_idx,
                        };
                        state.output = Some(
                            "Select network (↑↓ to navigate, Enter to confirm, Esc to cancel)"
                                .to_string(),
                        );
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
                        let network_names: Vec<String> =
                            app.config.networks.keys().cloned().collect();
                        let network_name =
                            network_names.get(network_idx).cloned().unwrap_or_default();

                        // Get wallet name (None = use env var)
                        let wallet_names: Vec<String> =
                            app.config.wallets.keys().cloned().collect();
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
            Ok(Some(
                "Wallet has no keychain or env_var configured".to_string(),
            ))
        }
    })?;

    if let Some(msg) = status_msg {
        app.set_status(msg);
    }

    Ok(())
}

fn handle_add_wallet(app: &mut App) -> Result<()> {
    use crate::config::{Defaults, WalletConfig, get_private_key, store_private_key};
    use dialoguer::{Confirm, Input, Password};

    let (wallet_name, private_key, label, set_default) = with_restored_terminal(|| {
        let wallet_name_input: String = Input::new().with_prompt("Enter wallet name").interact()?;
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
        let clean_key = private_key
            .trim()
            .strip_prefix("0x")
            .unwrap_or(private_key.trim());
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
            tracing::info!(
                "Private key stored and verified for wallet: {}",
                wallet_name
            );
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
                    .with_prompt(format!(
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
                    .with_prompt(format!(
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
                    .with_prompt(format!("Delete API key for '{}'?", service_name))
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
