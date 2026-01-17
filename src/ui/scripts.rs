use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::{App, ScriptPhase, ScriptsState};

pub fn draw(frame: &mut Frame, app: &App, state: &ScriptsState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    draw_scripts_list(frame, app, state, chunks[0]);
    draw_script_output(frame, app, state, chunks[1]);
}

fn draw_scripts_list(frame: &mut Frame, app: &App, state: &ScriptsState, area: Rect) {
    let scripts = &app.scripts.scripts;

    if scripts.is_empty() {
        let paragraph = Paragraph::new(
            "No scripts found.\n\nCreate scripts in the `script/` directory\nwith the `.s.sol` extension.",
        )
        .style(Style::default().fg(Color::DarkGray))
        .block(
            Block::default()
                .title(" Scripts ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        )
        .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
        return;
    }

    let items: Vec<ListItem> = scripts
        .iter()
        .enumerate()
        .map(|(i, script)| {
            let style = if i == state.selected_script {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            let name_span = Span::styled(&script.name, style.add_modifier(Modifier::BOLD));

            let desc_line = if let Some(desc) = &script.description {
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(desc, Style::default().fg(Color::DarkGray)),
                ])
            } else {
                Line::from("")
            };

            ListItem::new(vec![Line::from(name_span), desc_line])
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(state.selected_script));

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Scripts ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        )
        .highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn draw_script_output(frame: &mut Frame, app: &App, state: &ScriptsState, area: Rect) {
    match &state.phase {
        ScriptPhase::SelectScript => {
            let content = if let Some(output) = &state.output {
                output.clone()
            } else {
                "Select a script and press Enter to run it.\n\n\
                 Scripts will be executed with `forge script`.\n\
                 Configure networks in the Config view."
                    .to_string()
            };

            let paragraph = Paragraph::new(content)
                .style(Style::default().fg(Color::DarkGray))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title(" Output ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Blue)),
                );
            frame.render_widget(paragraph, area);
        }

        ScriptPhase::SelectNetwork { selected } => {
            draw_selection_list(
                frame,
                area,
                " Select Network ",
                &app.config.networks.keys().cloned().collect::<Vec<_>>(),
                *selected,
                "↑↓ navigate • Enter confirm • Esc cancel",
            );
        }

        ScriptPhase::SelectWallet { selected, .. } => {
            let mut wallet_options = vec!["(use PRIVATE_KEY env var)".to_string()];
            wallet_options.extend(app.config.wallets.keys().cloned());

            draw_selection_list(
                frame,
                area,
                " Select Wallet ",
                &wallet_options,
                *selected,
                "↑↓ navigate • Enter run • Esc back",
            );
        }

        ScriptPhase::Running => {
            let content = state.output.as_deref().unwrap_or("Running script...");

            let paragraph = Paragraph::new(content)
                .style(Style::default().fg(Color::Yellow))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title(" Output (Esc to dismiss) ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Yellow)),
                );
            frame.render_widget(paragraph, area);
        }
    }
}

fn draw_selection_list(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    items: &[String],
    selected: usize,
    help_text: &str,
) {
    let list_items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let style = if i == selected {
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(Span::styled(format!("  {}  ", item), style)))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(selected));

    // Split area for list and help text
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(2)])
        .split(area);

    let list = List::new(list_items)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(Style::default().bg(Color::Blue));

    frame.render_stateful_widget(list, chunks[0], &mut list_state);

    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(help, chunks[1]);
}
