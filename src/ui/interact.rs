use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::{App, InteractFocus, InteractState};
use crate::contracts::ContractFunction;

pub fn draw(frame: &mut Frame, app: &App, state: &InteractState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    draw_deployments_list(frame, app, state, chunks[0]);
    draw_right_panel(frame, app, state, chunks[1]);
}

fn draw_deployments_list(frame: &mut Frame, app: &App, state: &InteractState, area: Rect) {
    let deployments = &app.deployments.deployments;
    let is_focused = matches!(state.focus, InteractFocus::Deployments);

    let border_color = if is_focused { Color::Cyan } else { Color::Blue };
    let title = if is_focused {
        " Deployments [active] "
    } else {
        " Deployments "
    };

    if deployments.is_empty() {
        let paragraph =
            Paragraph::new("No deployments found.\n\nRun `forge script` to deploy contracts.")
                .style(Style::default().fg(Color::DarkGray))
                .block(
                    Block::default()
                        .title(title)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(border_color)),
                )
                .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
        return;
    }

    let items: Vec<ListItem> = deployments
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let is_selected = i == state.selected_deployment;
            let style = if is_selected && is_focused {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else if is_selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            ListItem::new(Line::from(vec![
                Span::styled(&d.name, style.add_modifier(Modifier::BOLD)),
                Span::styled(format!(" ({})", d.network), Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(state.selected_deployment));

    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        )
        .highlight_style(if is_focused {
            Style::default().bg(Color::Blue)
        } else {
            Style::default().bg(Color::DarkGray)
        })
        .highlight_symbol(if is_focused { "▶ " } else { "  " });

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn draw_right_panel(frame: &mut Frame, app: &App, state: &InteractState, area: Rect) {
    // If we're in input mode, show input panel, otherwise show functions + result
    if matches!(state.focus, InteractFocus::Inputs) {
        draw_input_panel(frame, app, state, area);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area);

        draw_functions(frame, app, state, chunks[0]);
        draw_result(frame, state, chunks[1]);
    }
}

fn draw_functions(frame: &mut Frame, app: &App, state: &InteractState, area: Rect) {
    let deployment = app.deployments.deployments.get(state.selected_deployment);
    let is_focused = matches!(state.focus, InteractFocus::Functions);

    let border_color = if is_focused { Color::Cyan } else { Color::Blue };
    let title = if is_focused {
        " Functions [active] "
    } else {
        " Functions (Tab/→ to focus) "
    };

    if deployment.is_none() || deployment.map(|d| d.functions.is_empty()).unwrap_or(true) {
        let msg = if deployment.is_none() {
            "Select a deployment"
        } else {
            "No functions found in ABI"
        };

        let paragraph = Paragraph::new(msg)
            .style(Style::default().fg(Color::DarkGray))
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color)),
            );
        frame.render_widget(paragraph, area);
        return;
    }

    let deployment = deployment.unwrap();
    let items: Vec<ListItem> = deployment
        .functions
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let is_selected = i == state.selected_function;
            let style = if is_selected && is_focused {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else if is_selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            let state_badge = match f.state_mutability.as_str() {
                "view" | "pure" => Span::styled("[R] ", Style::default().fg(Color::Green)),
                _ => Span::styled("[W] ", Style::default().fg(Color::Yellow)),
            };

            ListItem::new(Line::from(vec![
                state_badge,
                Span::styled(&f.name, style.add_modifier(Modifier::BOLD)),
                Span::styled(format_function_signature(f), Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(state.selected_function));

    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        )
        .highlight_style(if is_focused {
            Style::default().bg(Color::Blue)
        } else {
            Style::default().bg(Color::DarkGray)
        })
        .highlight_symbol(if is_focused { "▶ " } else { "  " });

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn draw_input_panel(frame: &mut Frame, app: &App, state: &InteractState, area: Rect) {
    let deployment = app.deployments.deployments.get(state.selected_deployment);
    let func = deployment.and_then(|d| d.functions.get(state.selected_function));

    let Some(func) = func else {
        return;
    };

    let mut lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled("Function: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&func.name, Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Enter parameters (Tab/↑↓ to navigate, Enter to submit, Esc to cancel):",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
    ];

    for (i, input) in func.inputs.iter().enumerate() {
        let is_current = i == state.current_input;
        let value = state.input_values.get(i).map(|s| s.as_str()).unwrap_or("");

        let label_style = if is_current {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let input_style = if is_current {
            Style::default().bg(Color::DarkGray).fg(Color::White)
        } else {
            Style::default()
        };

        let cursor = if is_current { "█" } else { "" };

        lines.push(Line::from(vec![
            Span::styled(if is_current { "▶ " } else { "  " }, label_style),
            Span::styled(format!("{} ", input.name), label_style),
            Span::styled(format!("({})", input.param_type), Style::default().fg(Color::DarkGray)),
        ]));

        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(format!("{}{}", value, cursor), input_style),
        ]));

        lines.push(Line::from(""));
    }

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Enter Parameters ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: false });

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
        format!(" → {}", outputs.join(", "))
    };

    format!("({}){}", params.join(", "), returns)
}

fn draw_result(frame: &mut Frame, state: &InteractState, area: Rect) {
    let (content, style) = if let Some(err) = &state.error {
        (err.as_str(), Style::default().fg(Color::Red))
    } else if let Some(result) = &state.result {
        (result.as_str(), Style::default().fg(Color::Green))
    } else {
        (
            "Select a function and press Enter to call it",
            Style::default().fg(Color::DarkGray),
        )
    };

    let paragraph = Paragraph::new(content)
        .style(style)
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(" Result ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        );

    frame.render_widget(paragraph, area);
}
