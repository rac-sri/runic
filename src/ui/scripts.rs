use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::{App, ScriptsState};

pub fn draw(frame: &mut Frame, app: &App, state: &ScriptsState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    draw_scripts_list(frame, app, state, chunks[0]);
    draw_script_output(frame, state, chunks[1]);
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

fn draw_script_output(frame: &mut Frame, state: &ScriptsState, area: Rect) {
    let (content, style) = if state.running {
        ("Running script...", Style::default().fg(Color::Yellow))
    } else if let Some(output) = &state.output {
        (output.as_str(), Style::default().fg(Color::Green))
    } else {
        (
            "Select a script and press Enter to run it.\n\n\
             Scripts will be executed with `forge script`.\n\
             Configure networks in the Config view.",
            Style::default().fg(Color::DarkGray),
        )
    };

    let paragraph = Paragraph::new(content)
        .style(style)
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(" Output ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        );

    frame.render_widget(paragraph, area);
}
