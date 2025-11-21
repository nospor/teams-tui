use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use crate::app::App;

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Min(3),
                Constraint::Length(3),
            ]
            .as_ref(),
        )
        .split(f.area());

    // Chat list
    let items: Vec<ListItem> = app
        .chats
        .iter()
        .enumerate()
        .map(|(i, chat)| {
            let display_name = chat.topic.as_ref()
                .map(|t| t.as_str())
                .unwrap_or(&chat.id);
            
            let style = if i == app.selected_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let content = Line::from(vec![
                Span::styled(format!("[{}] ", chat.chat_type), Style::default().fg(Color::Cyan)),
                Span::styled(display_name, style),
            ]);
            
            ListItem::new(content)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title("Teams Chats (↑/↓ to navigate, q to quit)")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White))
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        );

    f.render_widget(list, chunks[0]);

    // Status bar
    let status = Paragraph::new(app.status.as_str())
        .block(
            Block::default()
                .title("Status")
                .borders(Borders::ALL)
        )
        .style(Style::default().fg(Color::Green));

    f.render_widget(status, chunks[1]);
}
