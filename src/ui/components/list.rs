use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, StatefulWidget},
};

/// A reusable selectable list component
pub struct SelectableList<'a> {
    items: Vec<ListItem<'a>>,
    title: &'a str,
    selected: usize,
}

impl<'a> SelectableList<'a> {
    pub fn new(items: Vec<ListItem<'a>>, title: &'a str) -> Self {
        Self {
            items,
            title,
            selected: 0,
        }
    }

    pub fn selected(mut self, index: usize) -> Self {
        self.selected = index;
        self
    }
}

impl StatefulWidget for SelectableList<'_> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.select(Some(self.selected));

        let list = List::new(self.items)
            .block(
                Block::default()
                    .title(self.title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue)),
            )
            .highlight_style(Style::default().bg(Color::DarkGray))
            .highlight_symbol("> ");

        StatefulWidget::render(list, area, buf, state);
    }
}
