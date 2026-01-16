use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::{App, InteractState};
use crate::contracts::ContractFunction;

pub fn draw(frame: &mut Frame, app: &App, state: &InteractState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    draw_deployments_list(frame, app, state, chunks[0]);
    draw_function_details(frame, app, state, chunks[1]);
}

fn draw_deployments_list(frame: &mut Frame, app: &App, state: &InteractState, area: Rect) {
    let deployments = &app.deployments.deployments;

    if deployments.is_empty() {
        let paragraph = Paragraph::new("No deployments found.\n\nRun `forge script` to deploy contracts.")
            .style(Style::default().fg(Color::DarkGray))
            .block(
                Block::default()
                    .title(" Deployments ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue)),
            )
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
        return;
    }

    let items: Vec<ListItem> = deployments
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let style = if i == state.selected_deployment {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            ListItem::new(Line::from(vec![
                Span::styled(&d.name, style.add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!(" ({})", d.network),
                    style.fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(state.selected_deployment));

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Deployments ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        )
        .highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn draw_function_details(frame: &mut Frame, app: &App, state: &InteractState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    draw_functions(frame, app, state, chunks[0]);
    draw_result(frame, state, chunks[1]);
}

fn draw_functions(frame: &mut Frame, app: &App, state: &InteractState, area: Rect) {
    let deployment = app.deployments.deployments.get(state.selected_deployment);

    let content = if let Some(deployment) = deployment {
        if deployment.functions.is_empty() {
            vec![Line::from(Span::styled(
                "No functions found in ABI",
                Style::default().fg(Color::DarkGray),
            ))]
        } else {
            deployment
                .functions
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    let is_selected = i == state.selected_function;
                    let style = if is_selected {
                        Style::default().bg(Color::DarkGray)
                    } else {
                        Style::default()
                    };

                    let state_badge = match f.state_mutability.as_str() {
                        "view" | "pure" => Span::styled("[R] ", Style::default().fg(Color::Green)),
                        _ => Span::styled("[W] ", Style::default().fg(Color::Yellow)),
                    };

                    Line::from(vec![
                        state_badge,
                        Span::styled(&f.name, style.add_modifier(Modifier::BOLD)),
                        Span::styled(
                            format_function_signature(f),
                            style.fg(Color::DarkGray),
                        ),
                    ])
                })
                .collect()
        }
    } else {
        vec![Line::from(Span::styled(
            "Select a deployment",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let paragraph = Paragraph::new(content).block(
        Block::default()
            .title(" Functions ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue)),
    );

    frame.render_widget(paragraph, area);
}

fn format_function_signature(f: &ContractFunction) -> String {
    let params: Vec<String> = f
        .inputs
        .iter()
        .map(|p| format!("{}: {}", p.name, p.param_type))
        .collect();

    let outputs: Vec<String> = f.outputs.iter().map(|o| o.param_type.clone()).collect();

    let returns = if outputs.is_empty() {
        String::new()
    } else {
        format!(" -> {}", outputs.join(", "))
    };

    format!("({}){}", params.join(", "), returns)
}

fn draw_result(frame: &mut Frame, state: &InteractState, area: Rect) {
    let content = if let Some(result) = &state.result {
        Paragraph::new(result.as_str())
            .style(Style::default().fg(Color::Green))
            .wrap(Wrap { trim: true })
    } else if state.input_mode {
        Paragraph::new("Enter function parameters...")
            .style(Style::default().fg(Color::Yellow))
    } else {
        Paragraph::new("Press Enter to call the selected function")
            .style(Style::default().fg(Color::DarkGray))
    };

    let block = Block::default()
        .title(" Result ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

    frame.render_widget(content.block(block), area);
}
