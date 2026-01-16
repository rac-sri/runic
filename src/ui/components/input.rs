use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Widget},
};

/// A text input component
pub struct TextInput<'a> {
    value: &'a str,
    label: &'a str,
    placeholder: &'a str,
    focused: bool,
    cursor_position: usize,
}

impl<'a> TextInput<'a> {
    pub fn new(value: &'a str, label: &'a str) -> Self {
        Self {
            value,
            label,
            placeholder: "",
            focused: false,
            cursor_position: value.len(),
        }
    }

    pub fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn cursor_position(mut self, pos: usize) -> Self {
        self.cursor_position = pos;
        self
    }
}

impl Widget for TextInput<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_color = if self.focused {
            Color::Cyan
        } else {
            Color::DarkGray
        };

        let display_text = if self.value.is_empty() {
            Span::styled(self.placeholder, Style::default().fg(Color::DarkGray))
        } else {
            Span::raw(self.value)
        };

        let paragraph = Paragraph::new(display_text).block(
            Block::default()
                .title(self.label)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        );

        paragraph.render(area, buf);

        // Render cursor if focused
        if self.focused && area.width > 2 && area.height > 2 {
            let cursor_x = area.x + 1 + (self.cursor_position as u16).min(area.width - 3);
            let cursor_y = area.y + 1;

            if let Some(cell) = buf.cell_mut(Position::new(cursor_x, cursor_y)) {
                cell.set_style(Style::default().bg(Color::White).fg(Color::Black));
            }
        }
    }
}
