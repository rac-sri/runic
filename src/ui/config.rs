use ratatui::{
    prelude::*,
    text::Span,
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

use crate::app::App;

fn truncate_url(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(6),
            Constraint::Min(5),
            Constraint::Min(4),
            Constraint::Min(10),
        ])
        .split(area);

    draw_networks(frame, app, chunks[0]);
    draw_wallets(frame, app, chunks[1]);
    draw_api_keys(frame, app, chunks[2]);
    draw_keychain_management(frame, app, chunks[3]);
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

fn draw_wallets(frame: &mut Frame, app: &App, area: Rect) {
    let wallets = &app.config.wallets;

    if wallets.is_empty() {
        let help_text = format!(
            "No wallets configured.\n\n\
                 Add wallets to: {}\n\n\
                 Example:\n\
                 [wallets.dev]\n\
                 keychain = \"runic:dev_wallet\"\n\
                 label = \"Development Wallet\"",
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
                    .title(" Wallets ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            );

        frame.render_widget(paragraph, area);
        return;
    }

    let items: Vec<ListItem> = wallets
        .iter()
        .map(|(name, wallet)| {
            let keychain_status = if let Some(keychain_ref) = &wallet.keychain {
                let key = keychain_ref.strip_prefix("runic:").unwrap_or(keychain_ref);
                match crate::config::get_private_key(key).ok().flatten() {
                    Some(_) => "Stored (Keychain)",
                    None => "Missing (Keychain)",
                }
            } else {
                "No Keychain"
            };

            let label = wallet.label.as_deref().unwrap_or(name);
            ListItem::new(format!("{} {} ({})", keychain_status, label, name))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Wallets ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    frame.render_widget(list, area);
}

fn draw_keychain_management(frame: &mut Frame, _app: &App, area: Rect) {
    let help_text = "Keychain Management\n\n\
        Commands:\n\
        • Press 'k' to add new private key\n\
        • Press 'r' to add new RPC URL\n\
        • Press 'a' to add new API key\n\
        • Press 'd' to delete stored credentials\n\
        • Press 'e' to export/view private key\n\
        \n\
        Stored securely in OS keychain (service: runic)";

    let paragraph = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(" Keychain Management ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta)),
        );

    frame.render_widget(paragraph, area);
}

fn draw_api_keys(frame: &mut Frame, app: &App, area: Rect) {
    let api_keys = &app.config.api_keys;

    let items: Vec<ListItem> = api_keys
        .iter()
        .map(|(name, value)| {
            let is_keychain = value.starts_with("keychain:");
            let status = if is_keychain { "🔓" } else { "⚠️" };

            ListItem::new(format!("{} {}", status, name))
        })
        .collect();

    if items.is_empty() {
        let content = Paragraph::new("No API keys configured.\n\nAdd to config:\n[api_keys]\netherscan = \"keychain:etherscan\"")
            .style(Style::default().fg(Color::DarkGray))
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .title(" API Keys ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue)),
            );
        frame.render_widget(content, area);
    } else {
        let list = List::new(items).block(
            Block::default()
                .title(" API Keys ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        );
        frame.render_widget(list, area);
    }
}
