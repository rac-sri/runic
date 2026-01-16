mod components;
mod config;
mod home;
mod interact;
mod scripts;

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

use crate::app::{App, View};

/// Main draw function - dispatches to appropriate view
pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Content
            Constraint::Length(3), // Footer/status
        ])
        .split(frame.area());

    draw_header(frame, app, chunks[0]);

    match &app.view {
        View::Home => home::draw(frame, app, chunks[1]),
        View::Interact(state) => interact::draw(frame, app, state, chunks[1]),
        View::Scripts(state) => scripts::draw(frame, app, state, chunks[1]),
        View::Config => config::draw(frame, app, chunks[1]),
    }

    draw_footer(frame, app, chunks[2]);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let title = format!(
        " runic - {} ({}) ",
        app.project.name, app.project.project_type
    );

    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    frame.render_widget(block, area);
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let help_text = match &app.view {
        View::Home => "[i] Interact  [s] Scripts  [c] Config  [q] Quit",
        View::Interact(state) if state.input_mode => {
            "[Tab] Next field  [Enter] Submit  [Esc] Cancel"
        }
        View::Interact(_) => "[↑/k] Up  [↓/j] Down  [Enter] Select  [Esc] Back",
        View::Scripts(state) if state.running => "[Esc] Cancel",
        View::Scripts(_) => "[↑/k] Up  [↓/j] Down  [Enter] Run  [Esc] Back",
        View::Config => "[Esc] Back",
    };

    let status = if let Some(msg) = &app.status_message {
        format!(" {} │ {} ", msg, help_text)
    } else {
        format!(" {} ", help_text)
    };

    let paragraph = Paragraph::new(status)
        .style(Style::default().fg(Color::Gray))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );

    frame.render_widget(paragraph, area);
}
