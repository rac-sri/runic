use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::app::App;

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    draw_project_info(frame, app, chunks[0]);
    draw_quick_stats(frame, app, chunks[1]);
}

fn draw_project_info(frame: &mut Frame, app: &App, area: Rect) {
    let project = &app.project;

    let info_lines = vec![
        Line::from(vec![
            Span::styled("Type: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                project.project_type.to_string(),
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Root: ", Style::default().fg(Color::DarkGray)),
            Span::raw(project.root.display().to_string()),
        ]),
        Line::from(vec![
            Span::styled("Source: ", Style::default().fg(Color::DarkGray)),
            Span::raw(
                project
                    .src_dir
                    .strip_prefix(&project.root)
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| project.src_dir.display().to_string()),
            ),
        ]),
        Line::from(vec![
            Span::styled("Output: ", Style::default().fg(Color::DarkGray)),
            Span::raw(
                project
                    .out_dir
                    .strip_prefix(&project.root)
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| project.out_dir.display().to_string()),
            ),
        ]),
        Line::from(vec![
            Span::styled("Scripts: ", Style::default().fg(Color::DarkGray)),
            Span::raw(
                project
                    .script_dir
                    .strip_prefix(&project.root)
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| project.script_dir.display().to_string()),
            ),
        ]),
    ];

    let paragraph = Paragraph::new(info_lines).block(
        Block::default()
            .title(" Project ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue)),
    );

    frame.render_widget(paragraph, area);
}

fn draw_quick_stats(frame: &mut Frame, app: &App, area: Rect) {
    let deployments_count = app.deployments.deployments.len();
    let scripts_count = app.scripts.scripts.len();
    let networks_count = app.config.networks.len();

    let items: Vec<ListItem> = vec![
        ListItem::new(Line::from(vec![
            Span::styled(
                format!("{:>3}", deployments_count),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" deployments found"),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled(
                format!("{:>3}", scripts_count),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" scripts available"),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled(
                format!("{:>3}", networks_count),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" networks configured"),
        ])),
    ];

    let list = List::new(items).block(
        Block::default()
            .title(" Overview ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue)),
    );

    frame.render_widget(list, area);
}
