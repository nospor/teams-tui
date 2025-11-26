use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use crate::app::App;

pub fn draw(f: &mut Frame, app: &mut App) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Min(3),     // Main content
                Constraint::Length(3),  // Status
            ]
            .as_ref(),
        )
        .split(f.area());

    // Split main content horizontally: chats on left, messages on right
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(30),  // Chat list
                Constraint::Percentage(70),  // Messages
            ]
            .as_ref(),
        )
        .split(main_chunks[0]);

    // Split messages area vertically if in input mode
    let messages_chunks = if app.input_mode {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Min(3),      // Messages
                    Constraint::Length(3),   // Input field
                ]
                .as_ref(),
            )
            .split(content_chunks[1])
    } else {
        std::rc::Rc::from(vec![content_chunks[1]].into_boxed_slice())
    };

    // Chat list
    let items: Vec<ListItem> = app
        .chats
        .iter()
        .enumerate()
        .map(|(i, chat)| {
            let display_name = chat.cached_display_name.as_deref()
                .unwrap_or("Unknown");
            
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
                .title("Teams Chats (j↑/k↓ to navigate, q to quit)")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White))
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        );

    f.render_widget(list, content_chunks[0]);

    // Messages panel
    let messages_content = if app.loading_messages {
        vec![Line::from("Loading messages...")]
    } else if app.messages.is_empty() {
        vec![Line::from("Loading messages...")]
    } else {
        let width = messages_chunks[0].width.saturating_sub(2) as usize; // Account for borders
        let max_line_width = (width as f32 * 0.9) as usize; // Max 90% width for messages
        
        let mut lines = Vec::new();
        let mut last_sender: Option<String> = None;
        let mut last_message_time: Option<chrono::DateTime<chrono::FixedOffset>> = None;
        
        // Take 100 newest messages
        for msg in app.messages.iter().take(100).rev() { 
            let sender_name = msg.from.as_ref()
                .and_then(|f| f.user.as_ref())
                .and_then(|u| u.display_name.as_ref())
                .map(|s| s.as_str())
                .unwrap_or("Unknown");
            
            let current_time = chrono::DateTime::parse_from_rfc3339(&msg.created_date_time).ok();
            
            let is_me = app.current_user_name.as_ref().map_or(false, |me| sender_name == me);
            let same_sender = last_sender.as_deref() == Some(sender_name);
            
            let significant_time_gap = if let (Some(curr), Some(last)) = (current_time, last_message_time) {
                let curr_hour = curr.format("%Y-%m-%d %H").to_string();
                let last_hour = last.format("%Y-%m-%d %H").to_string();
                curr_hour != last_hour
            } else {
                false
            };
            
            let show_header = !same_sender || significant_time_gap;
            
            last_sender = Some(sender_name.to_string());
            last_message_time = current_time;
            
            // Format date: 2025-11-21T19:11:33 -> Nov-21 19:11
            let date_str = if let Some(dt) = current_time {
                dt.format("%b %d %H:%M").to_string()
            } else {
                msg.created_date_time.clone()
            };
            
            let content = msg.body.as_ref()
                .and_then(|b| b.content.as_ref())
                .map(|c| c.as_str())
                .unwrap_or("");
            
            // Strip HTML tags and extract text content
            let mut clean_content = content.to_string();
            
            // Remove attachment tags (quoted messages) - they're just metadata
            // Handle both self-closing <attachment ... /> and <attachment ...></attachment>
            let mut attachment_removed = String::new();
            let mut remaining = clean_content.as_str();
            
            while let Some(attach_start) = remaining.find("<attachment") {
                // Add text before the attachment tag
                attachment_removed.push_str(&remaining[..attach_start]);
                
                // Find the end of the opening tag
                if let Some(tag_end) = remaining[attach_start..].find('>') {
                    // Check if it's self-closing (ends with />)
                    let tag_str = &remaining[attach_start..attach_start + tag_end];
                    if tag_str.ends_with('/') {
                        // Self-closing: <attachment ... />
                        remaining = &remaining[attach_start + tag_end + 1..];
                    } else {
                        // Has closing tag: <attachment ...></attachment>
                        remaining = &remaining[attach_start + tag_end + 1..];
                        // Skip past closing </attachment> tag
                        if let Some(close_start) = remaining.find("</attachment>") {
                            remaining = &remaining[close_start + 13..]; // 13 = len("</attachment>")
                        }
                    }
                } else {
                    // Malformed tag, skip the <attachment part
                    attachment_removed.push_str(&remaining[..attach_start + 11]);
                    remaining = &remaining[attach_start + 11..];
                }
            }
            
            // Add remaining text
            attachment_removed.push_str(remaining);
            clean_content = attachment_removed;
            
            // Extract emoji alt text: <emoji ... alt="😅" ...> -> 😅
            // Process emoji tags by finding them and replacing with alt text
            let mut emoji_processed = String::new();
            remaining = clean_content.as_str();
            
            while let Some(emoji_start) = remaining.find("<emoji") {
                // Add text before the emoji tag
                emoji_processed.push_str(&remaining[..emoji_start]);
                
                // Find the end of the opening tag
                if let Some(tag_end) = remaining[emoji_start..].find('>') {
                    let tag_str = &remaining[emoji_start..emoji_start + tag_end + 1];
                    
                    // Extract alt attribute value
                    if let Some(alt_start) = tag_str.find("alt=\"") {
                        let alt_value_start = alt_start + 5;
                        if let Some(alt_end) = tag_str[alt_value_start..].find('"') {
                            let emoji = &tag_str[alt_value_start..alt_value_start + alt_end];
                            emoji_processed.push_str(emoji);
                        }
                    }
                    
                    // Skip past the opening tag
                    remaining = &remaining[emoji_start + tag_end + 1..];
                    
                    // Skip past closing </emoji> tag if present
                    if remaining.starts_with("</emoji") {
                        if let Some(close_end) = remaining.find('>') {
                            remaining = &remaining[close_end + 1..];
                        }
                    }
                } else {
                    // Malformed tag, skip the <emoji part
                    emoji_processed.push_str(&remaining[..emoji_start + 6]);
                    remaining = &remaining[emoji_start + 6..];
                }
            }
            
            // Add remaining text
            emoji_processed.push_str(remaining);
            clean_content = emoji_processed;
            
            // Handle HTML entities
            clean_content = clean_content
                .replace("&nbsp;", " ")
                .replace("&amp;", "&")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"")
                .replace("&#39;", "'")
                .replace("&apos;", "'")
                .replace("&#160;", " ")
                .replace("&nbsp", " ");
            
            // Convert block-level tags to newlines
            clean_content = clean_content
                .replace("</p>", "\n")
                .replace("<p>", "")
                .replace("</div>", "\n")
                .replace("<div>", "")
                .replace("</li>", "\n")
                .replace("<li>", "")
                .replace("<br>", "\n")
                .replace("<br/>", "\n")
                .replace("<br />", "\n")
                .replace("</br>", "\n");
            
            // Remove remaining HTML tags
            let mut no_html = String::new();
            let mut inside_tag = false;
            
            for c in clean_content.chars() {
                if c == '<' {
                    inside_tag = true;
                } else if c == '>' {
                    inside_tag = false;
                } else if !inside_tag {
                    no_html.push(c);
                }
            }
            
            // Clean up whitespace: limit consecutive newlines to 2
            let mut final_content = String::new();
            let mut consecutive_newlines = 0;
            
            for c in no_html.chars() {
                if c == '\n' {
                    consecutive_newlines += 1;
                    if consecutive_newlines <= 2 {
                        final_content.push(c);
                    }
                } else {
                    consecutive_newlines = 0;
                    final_content.push(c);
                }
            }
            
            // Trim leading/trailing whitespace
            let final_content = final_content.trim();

            // Wrap text manually, preserving newlines
            let mut wrapped_lines = Vec::new();
            
            if final_content.is_empty() {
                // Empty content - still show one empty line so message appears
                wrapped_lines.push(String::new());
            } else {
                for line in final_content.lines() {
                    let mut current_line = String::new();
                    
                    for word in line.split_whitespace() {
                        if current_line.len() + word.len() + 1 > max_line_width {
                            wrapped_lines.push(current_line);
                            current_line = String::from(word);
                        } else {
                            if !current_line.is_empty() {
                                current_line.push(' ');
                            }
                            current_line.push_str(word);
                        }
                    }
                    if !current_line.is_empty() {
                        wrapped_lines.push(current_line);
                    }
                }
                
                // Ensure at least one line exists
                if wrapped_lines.is_empty() {
                    wrapped_lines.push(String::new());
                }
            }

            // Header (if different sender or significant time gap)
            if show_header {
                // Add extra spacing before new group (unless it's the first message)
                if !lines.is_empty() {
                    lines.push(Line::from(""));
                }

                let header = if is_me {
                    format!("{} {}", date_str, "Me")
                } else {
                    format!("{} {}", sender_name, date_str)
                };

                if is_me {
                    // Right aligned header
                    let padding = width.saturating_sub(header.len());
                    let pad_str = " ".repeat(padding);
                    lines.push(Line::from(vec![
                        Span::raw(pad_str),
                        Span::styled(header, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    ]));
                } else {
                    // Left aligned header
                    lines.push(Line::from(vec![
                        Span::styled(header, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    ]));
                }
            }

            // Message body
            if is_me {
                // Right aligned body
                for line in wrapped_lines {
                    let padding = width.saturating_sub(line.len());
                    let pad_str = " ".repeat(padding);
                    lines.push(Line::from(vec![
                        Span::raw(pad_str),
                        Span::raw(line),
                    ]));
                }
            } else {
                // Left aligned body
                for line in wrapped_lines {
                    lines.push(Line::from(line));
                }
            }
        }
        
        lines
    };

    // Calculate scroll
    let total_lines = messages_content.len() as u16;
    let viewport_height = messages_chunks[0].height.saturating_sub(2); // Borders
    
    // Calculate max scroll: if we have more lines than viewport, scroll to show bottom
    // The newest messages are at the bottom of the content (after .rev(), they're last in lines vector)
    if total_lines > viewport_height {
        // To see the last line (index total_lines-1), we need to scroll: total_lines - viewport_height
        // This positions the viewport so the last line is visible at the bottom
        app.max_scroll = total_lines.saturating_sub(viewport_height);
    } else {
        app.max_scroll = 0; // No scrolling needed if all fits
    }
    
    // Always snap to bottom when loading new messages or if explicitly requested
    // This shows the newest messages at the bottom
    if app.snap_to_bottom {
        // Calculate scroll offset to ensure the last line is fully visible
        // Scroll enough so that the last line (index total_lines-1) appears at the bottom of viewport
        if total_lines > viewport_height {
            // Scroll to show the last viewport_height lines
            // Add extra margin (3-5 lines) to ensure the last message is definitely visible
            // This accounts for potential wrapping, spacing, or calculation errors
            let extra_margin = 5u16; // Scroll a bit more than necessary
            app.scroll_offset = total_lines.saturating_sub(viewport_height).saturating_add(extra_margin);
            // Cap at total_lines to prevent overflow (though we should never reach this)
            app.scroll_offset = std::cmp::min(app.scroll_offset, total_lines.saturating_sub(1));
        } else {
            app.scroll_offset = 0;
        }
        // Update max_scroll to allow scrolling to this position
        app.max_scroll = std::cmp::max(app.max_scroll, app.scroll_offset);
    } else {
        // Clamp scroll offset to valid range, but allow the extra margin
        app.scroll_offset = std::cmp::min(app.scroll_offset, app.max_scroll);
    }

    let messages_widget = Paragraph::new(messages_content)
        .block(
            Block::default()
                .title(if app.input_mode { "Messages (ESC to cancel)" } else { "Messages (i to compose, PgUp/PgDn to scroll)" })
                .borders(Borders::ALL)
        )
        .wrap(ratatui::widgets::Wrap { trim: false })
        .scroll((app.scroll_offset, 0));

    f.render_widget(messages_widget, messages_chunks[0]);

    // Render input field if in input mode
    if app.input_mode {
        let input_widget = Paragraph::new(app.input_buffer.as_str())
            .block(
                Block::default()
                    .title("Type your message (Enter to send, ESC to cancel)")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green))
            )
            .style(Style::default().fg(Color::White));
        
        f.render_widget(input_widget, messages_chunks[1]);
        
        // Set cursor position
        f.set_cursor_position((
            messages_chunks[1].x + app.input_buffer.len() as u16 + 1,
            messages_chunks[1].y + 1,
        ));
    }

    // Status bar
    let status = Paragraph::new(app.status.as_str())
        .block(
            Block::default()
                .title("Status")
                .borders(Borders::ALL)
        )
        .style(Style::default().fg(Color::Green));

    f.render_widget(status, main_chunks[1]);
}
