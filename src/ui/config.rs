use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

use crate::app::App;

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    draw_networks(frame, app, chunks[0]);
    draw_credentials(frame, app, chunks[1]);
}

fn draw_networks(frame: &mut Frame, app: &App, area: Rect) {
    let networks = &app.config.networks;

    if networks.is_empty() {
        let help_text = format!(
            "No networks configured.\n\n\
             Add networks to: {}\n\n\
             Example:\n\
             [networks.sepolia]\n\
             rpc_url = \"https://...\"\n\
             chain_id = 11155111",
            app.config
                .config_path()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "~/.config/runic/config.toml".to_string())
        );

        let paragraph = Paragraph::new(help_text)
            .style(Style::default().fg(Color::DarkGray))
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .title(" Networks ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue)),
            );

        frame.render_widget(paragraph, area);
        return;
    }

    let items: Vec<ListItem> = networks
        .iter()
        .map(|(name, network)| {
            let is_default = app
                .config
                .defaults
                .as_ref()
                .is_some_and(|d| d.network.as_ref() == Some(name));

            let default_badge = if is_default {
                Span::styled(" (default)", Style::default().fg(Color::Green))
            } else {
                Span::raw("")
            };

            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(name, Style::default().add_modifier(Modifier::BOLD)),
                    default_badge,
                ]),
                Line::from(vec![
                    Span::styled("  Chain ID: ", Style::default().fg(Color::DarkGray)),
                    Span::raw(
                        network
                            .chain_id
                            .map(|id| id.to_string())
                            .unwrap_or_else(|| "?".to_string()),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  RPC: ", Style::default().fg(Color::DarkGray)),
                    Span::raw(truncate_url(&network.rpc_url, 30)),
                ]),
            ])
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(" Networks ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue)),
    );

    frame.render_widget(list, area);
}

fn draw_credentials(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    draw_api_keys(frame, app, chunks[0]);
    draw_wallets(frame, app, chunks[1]);
}

fn draw_api_keys(frame: &mut Frame, app: &App, area: Rect) {
    let api_keys = &app.config.api_keys;

    let items: Vec<ListItem> = api_keys
        .iter()
        .map(|(name, value)| {
            let is_keychain = value.starts_with("keychain:");
            let status = if is_keychain {
                Span::styled("keychain", Style::default().fg(Color::Green))
            } else {
                Span::styled("plaintext", Style::default().fg(Color::Yellow))
            };

            ListItem::new(Line::from(vec![
                Span::styled(name, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(": "),
                status,
            ]))
        })
        .collect();

    let content = if items.is_empty() {
        Paragraph::new("No API keys configured.\n\nAdd to config:\n[api_keys]\netherscan = \"keychain:etherscan\"")
            .style(Style::default().fg(Color::DarkGray))
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .title(" API Keys ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue)),
            )
    } else {
        let list = List::new(items).block(
            Block::default()
                .title(" API Keys ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        );
        frame.render_widget(list, area);
        return;
    };

    frame.render_widget(content, area);
}

fn draw_wallets(frame: &mut Frame, app: &App, area: Rect) {
    let wallets = &app.config.wallets;

    let items: Vec<ListItem> = wallets
        .iter()
        .map(|(name, wallet)| {
            let source = if wallet.keychain.is_some() {
                Span::styled("keychain", Style::default().fg(Color::Green))
            } else if wallet.env_var.is_some() {
                Span::styled("env", Style::default().fg(Color::Yellow))
            } else {
                Span::styled("unknown", Style::default().fg(Color::Red))
            };

            ListItem::new(Line::from(vec![
                Span::styled(name, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(": "),
                source,
            ]))
        })
        .collect();

    let content = if items.is_empty() {
        Paragraph::new(
            "No wallets configured.\n\n\
             Add to config:\n\
             [wallets.dev]\n\
             keychain = \"runic:dev_wallet\"\n\n\
             Private keys are stored\n\
             securely in OS keychain.",
        )
        .style(Style::default().fg(Color::DarkGray))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(" Wallets ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        )
    } else {
        let list = List::new(items).block(
            Block::default()
                .title(" Wallets ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        );
        frame.render_widget(list, area);
        return;
    };

    frame.render_widget(content, area);
}

fn truncate_url(url: &str, max_len: usize) -> String {
    if url.len() <= max_len {
        url.to_string()
    } else {
        format!("{}...", &url[..max_len - 3])
    }
}
