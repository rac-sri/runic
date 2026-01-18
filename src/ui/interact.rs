use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::{App, InteractFocus, InteractState, NetworkInfo};
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

            let proxy_indicator = if d.callable_address != d.address {
                " [P]"
            } else {
                ""
            };
            ListItem::new(Line::from(vec![
                Span::styled(&d.name, style.add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{} ({} #{})", proxy_indicator, d.network, d.chain_id),
                    Style::default().fg(if proxy_indicator.is_empty() {
                        Color::DarkGray
                    } else {
                        Color::Yellow
                    }),
                ),
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
    if matches!(state.focus, crate::app::InteractFocus::Inputs) {
        draw_input_panel(frame, app, state, area);
    } else if matches!(state.focus, crate::app::InteractFocus::WalletSelection) {
        draw_wallet_selection_panel(frame, app, state, area);
    } else if matches!(state.focus, crate::app::InteractFocus::AbiSelection) {
        draw_abi_selection_panel(frame, app, state, area);
    } else if matches!(state.focus, crate::app::InteractFocus::ImplementationPrompt) {
        draw_implementation_prompt_panel(frame, app, state, area);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area);

        draw_functions(frame, app, state, chunks[0]);
        draw_result(frame, app, state, chunks[1]);
    }
}

fn draw_abi_selection_panel(frame: &mut Frame, app: &App, state: &InteractState, area: Rect) {
    let deployments = &app.deployments.deployments;

    let items: Vec<ListItem> = deployments
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let is_selected = i == state.abi_selection_index;
            let style = if is_selected {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                Style::default()
            };

            ListItem::new(Line::from(vec![
                Span::styled(&d.name, style.add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!(" ({})", d.network),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(state.abi_selection_index));

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Select Implementation ABI ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(Style::default().bg(Color::Blue))
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn draw_implementation_prompt_panel(
    frame: &mut Frame,
    app: &App,
    state: &InteractState,
    area: Rect,
) {
    let deployments = &app.deployments.deployments;
    let current_deployment = deployments.get(state.selected_deployment);

    // Split into header section and list section
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(5)])
        .split(area);

    // Header with explanation
    let proxy_addr = current_deployment
        .map(|d| d.callable_address.as_str())
        .unwrap_or("unknown");
    let impl_addr = current_deployment
        .map(|d| d.address.as_str())
        .unwrap_or("unknown");

    let header_lines = vec![
        Line::from(vec![
            Span::styled("Proxy Contract Detected", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Proxy: ", Style::default().fg(Color::DarkGray)),
            Span::styled(proxy_addr, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("Implementation: ", Style::default().fg(Color::DarkGray)),
            Span::styled(impl_addr, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Select implementation ABI below, or press 's' to skip:",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let header = Paragraph::new(header_lines).block(
        Block::default()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .border_style(Style::default().fg(Color::Yellow)),
    );

    frame.render_widget(header, chunks[0]);

    // List of available ABIs
    let items: Vec<ListItem> = deployments
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let is_selected = i == state.abi_selection_index;
            let is_current = i == state.selected_deployment;
            let style = if is_selected {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                Style::default()
            };

            let current_marker = if is_current { " (current)" } else { "" };

            ListItem::new(Line::from(vec![
                Span::styled(&d.name, style.add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!(" ({}){}", d.network, current_marker),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(state.abi_selection_index));

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Select Implementation ABI (Enter to select, 's' to skip) ")
                .borders(Borders::BOTTOM | Borders::LEFT | Borders::RIGHT)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .highlight_style(Style::default().bg(Color::Blue))
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, chunks[1], &mut list_state);
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
                Span::styled(
                    format_function_signature(f),
                    Style::default().fg(Color::DarkGray),
                ),
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
        let value = state.input_values.get(i).map(|s: &String| s.as_str()).unwrap_or("");

        let label_style = if is_current {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
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
            Span::styled(
                format!("({})", input.param_type),
                Style::default().fg(Color::DarkGray),
            ),
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

fn draw_wallet_selection_panel(frame: &mut Frame, app: &App, state: &InteractState, area: Rect) {
    let wallet_names: Vec<String> = app.config.wallets.keys().cloned().collect();
    let is_focused = matches!(state.focus, crate::app::InteractFocus::WalletSelection);

    if wallet_names.is_empty() {
        let paragraph =
            Paragraph::new("No wallets configured.\n\nAdd a wallet in config (press 'c').")
                .style(Style::default().fg(Color::DarkGray))
                .block(
                    Block::default()
                        .title(" Select Wallet ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan)),
                )
                .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
        return;
    }

    let items: Vec<ListItem> = wallet_names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let is_selected = state.selected_wallet.as_ref() == Some(name);
            let style = if is_selected && is_focused {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else if is_selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            ListItem::new(Line::from(vec![Span::styled(
                name,
                style.add_modifier(Modifier::BOLD),
            )]))
        })
        .collect();

    let mut list_state = ListState::default();
    if let Some(selected) = &state.selected_wallet {
        list_state.select(wallet_names.iter().position(|n| n == selected));
    }

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Select Wallet for Write Transaction ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(if is_focused {
            Style::default().bg(Color::Blue)
        } else {
            Style::default().bg(Color::DarkGray)
        })
        .highlight_symbol(if is_focused { "▶ " } else { "  " });

    frame.render_stateful_widget(list, area, &mut list_state);
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

fn draw_result(frame: &mut Frame, app: &App, state: &InteractState, area: Rect) {
    let mut lines: Vec<Line> = vec![];

    let deployment = app.deployments.deployments.get(state.selected_deployment);
    let func = deployment.and_then(|d| d.functions.get(state.selected_function));

    if let (Some(deployment), Some(func)) = (deployment, func) {
        lines.push(Line::from(vec![
            Span::styled("Contract: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                &deployment.name,
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]));

        lines.push(Line::from(vec![
            Span::styled("Address: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                &deployment.callable_address,
                Style::default().fg(Color::Cyan),
            ),
        ]));

        // Always show the deployment's network/chain info
        lines.push(Line::from(vec![
            Span::styled("Network: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} (chain {})", deployment.network, deployment.chain_id),
                Style::default().fg(Color::Cyan),
            ),
        ]));

        if deployment.callable_address != deployment.address {
            lines.push(Line::from(vec![
                Span::styled("Proxy: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    "Calls routed through proxy",
                    Style::default().fg(Color::Yellow),
                ),
            ]));
        }

        // Show RPC URL only after a call has been made
        if let Some(network_info) = &state.network_info {
            lines.push(Line::from(vec![
                Span::styled("RPC: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    network_info.rpc_url.clone(),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }

        lines.push(Line::from(vec![
            Span::styled("Function: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&func.name, Style::default().add_modifier(Modifier::BOLD)),
        ]));

        lines.push(Line::from(""));

        if !state.input_values.is_empty() {
            lines.push(Line::from(Span::styled(
                "Inputs:",
                Style::default().add_modifier(Modifier::BOLD),
            )));

            for (i, input) in func.inputs.iter().enumerate() {
                if let Some(value) = state.input_values.get(i) {
                    let value: &String = value;
                    if !value.is_empty() {
                        lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::styled(
                                format!("{}: ", input.name),
                                Style::default().fg(Color::Cyan),
                            ),
                            Span::styled(value, Style::default()),
                        ]));
                    }
                }
            }

            lines.push(Line::from(""));
        }

        lines.push(Line::from(""));

        match &state.call_status {
            crate::app::CallStatus::Idle => {
                lines.push(Line::from(Span::styled(
                    "Select a function and press Enter to call it",
                    Style::default().fg(Color::DarkGray),
                )));
            }
            crate::app::CallStatus::Preparing => {
                lines.push(Line::from(vec![
                    Span::styled("● ", Style::default().fg(Color::Yellow)),
                    Span::styled("Preparing call...", Style::default().fg(Color::Yellow)),
                ]));
            }
            crate::app::CallStatus::Connecting => {
                lines.push(Line::from(vec![
                    Span::styled("● ", Style::default().fg(Color::Yellow)),
                    Span::styled("Connecting to RPC...", Style::default().fg(Color::Yellow)),
                ]));
            }
            crate::app::CallStatus::Executing => {
                lines.push(Line::from(vec![
                    Span::styled("● ", Style::default().fg(Color::Yellow)),
                    Span::styled("Executing call...", Style::default().fg(Color::Yellow)),
                ]));
            }
            crate::app::CallStatus::Completed => {
                if let Some(result) = &state.result {
                    lines.push(Line::from(vec![
                        Span::styled("● ", Style::default().fg(Color::Green)),
                        Span::styled("Result: ", Style::default().fg(Color::Green)),
                        Span::styled(result, Style::default().fg(Color::Green)),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::styled("● ", Style::default().fg(Color::Green)),
                        Span::styled("Call completed", Style::default().fg(Color::Green)),
                    ]));
                }
            }
            crate::app::CallStatus::Failed(msg) => {
                lines.push(Line::from(vec![
                    Span::styled("✗ ", Style::default().fg(Color::Red)),
                    Span::styled("Failed: ", Style::default().fg(Color::Red)),
                    Span::styled(msg, Style::default().fg(Color::Red)),
                ]));
            }
        }

        if let Some(error) = &state.error {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Error Details:",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )));

            // Split error into lines to ensure full display
            for error_line in error.lines() {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(error_line, Style::default().fg(Color::Red)),
                ]));
            }
        }
    } else {
        lines.push(Line::from(Span::styled(
            "Select a deployment and function to call",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true }).block(
        Block::default()
            .title(" Result ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue)),
    );

    frame.render_widget(paragraph, area);
}
