use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use eyre::Result;
use ratatui::{prelude::*, Terminal};

use crate::config::AppConfig;
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
    pub scripts: ScriptManager,
    pub status_message: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InteractState {
    pub selected_deployment: usize,
    pub selected_function: usize,
    pub input_mode: bool,
    pub input_values: Vec<String>,
    pub current_input: usize,
    pub result: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScriptsState {
    pub selected_script: usize,
    pub running: bool,
    pub output: Option<String>,
}

impl App {
    pub fn new(project: Project) -> Result<Self> {
        let config = AppConfig::load()?;
        let deployments = DeploymentManager::new(&project);
        let scripts = ScriptManager::new(&project);

        Ok(Self {
            project,
            config,
            view: View::Home,
            should_quit: false,
            deployments,
            scripts,
            status_message: None,
        })
    }

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

    // Create app state
    let mut app = App::new(project)?;

    // Scan for deployments and scripts
    app.deployments.scan()?;
    app.scripts.scan()?;

    // Run main loop
    let result = run_app(&mut terminal, &mut app).await;

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

async fn run_app(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<()> {
    loop {
        // Draw UI
        terminal.draw(|frame| ui::draw(frame, app))?;

        // Handle events with timeout for async operations
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Global quit: Ctrl+C or q from home
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c')
                {
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
    let state = match &mut app.view {
        View::Interact(state) => state,
        _ => return,
    };

    if state.input_mode {
        // Handle text input
        match key {
            KeyCode::Esc => state.input_mode = false,
            KeyCode::Enter => {
                if state.current_input + 1 < state.input_values.len() {
                    state.current_input += 1;
                } else {
                    // Execute the call
                    state.input_mode = false;
                    // TODO: Execute contract call
                    state.result = Some("Call executed (mock)".to_string());
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
            KeyCode::Tab => {
                if state.current_input + 1 < state.input_values.len() {
                    state.current_input += 1;
                } else {
                    state.current_input = 0;
                }
            }
            KeyCode::BackTab => {
                if state.current_input > 0 {
                    state.current_input -= 1;
                } else {
                    state.current_input = state.input_values.len().saturating_sub(1);
                }
            }
            _ => {}
        }
    } else {
        match key {
            KeyCode::Esc => app.view = View::Home,
            KeyCode::Up | KeyCode::Char('k') => {
                state.selected_deployment = state.selected_deployment.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = app.deployments.deployments.len().saturating_sub(1);
                state.selected_deployment = (state.selected_deployment + 1).min(max);
            }
            KeyCode::Enter => {
                // Enter input mode for function params
                if !app.deployments.deployments.is_empty() {
                    state.input_mode = true;
                }
            }
            _ => {}
        }
    }
}

async fn handle_scripts_input(app: &mut App, key: KeyCode) {
    let state = match &mut app.view {
        View::Scripts(state) => state,
        _ => return,
    };

    if state.running {
        // Can only cancel while running
        if key == KeyCode::Esc {
            state.running = false;
        }
        return;
    }

    match key {
        KeyCode::Esc => app.view = View::Home,
        KeyCode::Up | KeyCode::Char('k') => {
            state.selected_script = state.selected_script.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let max = app.scripts.scripts.len().saturating_sub(1);
            state.selected_script = (state.selected_script + 1).min(max);
        }
        KeyCode::Enter => {
            // Run selected script
            if let Some(script) = app.scripts.scripts.get(state.selected_script) {
                state.running = true;
                // TODO: Actually run the script
                state.output = Some(format!("Would run: {}", script.name));
                state.running = false;
            }
        }
        _ => {}
    }
}

fn handle_config_input(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Esc | KeyCode::Char('q') => app.view = View::Home,
        // TODO: Config editing
        _ => {}
    }
}
