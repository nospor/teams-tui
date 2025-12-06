use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use crate::app::App;

// Get icon based on file extension or content type
fn get_attachment_icon(name: &str, content_type: Option<&str>) -> &'static str {
    // First try content type
    if let Some(ct) = content_type {
        if ct.starts_with("image/") {
            return "🖼️";
        } else if ct == "application/pdf" {
            return "📄";
        } else if ct.contains("excel") || ct.contains("spreadsheet") {
            return "📊";
        } else if ct.contains("word") || ct.contains("document") {
            return "📝";
        } else if ct.contains("powerpoint") || ct.contains("presentation") {
            return "📊";
        } else if ct.starts_with("video/") {
            return "🎥";
        } else if ct.starts_with("audio/") {
            return "🎵";
        } else if ct.contains("zip") || ct.contains("archive") {
            return "📦";
        }
    }
    
    // Fall back to file extension
    if let Some(dot_pos) = name.rfind('.') {
        let ext = name[dot_pos + 1..].to_lowercase();
        match ext.as_str() {
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "svg" | "webp" => "🖼️",
            "pdf" => "📄",
            "doc" | "docx" => "📝",
            "xls" | "xlsx" | "csv" => "📊",
            "ppt" | "pptx" => "📊",
            "mp4" | "avi" | "mov" | "mkv" | "webm" => "🎥",
            "mp3" | "wav" | "ogg" | "flac" => "🎵",
            "zip" | "rar" | "7z" | "tar" | "gz" => "📦",
            "txt" => "📄",
            "html" | "htm" => "🌐",
            "json" | "xml" => "📋",
            _ => "📎", // Default attachment icon
        }
    } else {
        "📎" // No extension, use default
    }
}

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

    // Visual bell effect: flash various UI elements or the background
    let is_flashing = app.visual_bell_until
        .map(|until| std::time::Instant::now() < until)
        .unwrap_or(false);

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
                    Constraint::Min(3),       // Input field (min 3 lines for multiline)
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
            
            
            // Parse message content into segments (Text, Image, Attachment) to preserve order
            #[derive(Debug)]
            enum MessageSegment {
                Text(String),
                Image(ImageInfo),
                Attachment(String), // Attachment ID
            }

            #[derive(Clone, Debug)]
            struct ImageInfo {
                src: Option<String>,
                alt: Option<String>,
            }

            let mut segments = Vec::new();
            // Use existing content variable
            let mut remaining = content;
            
            // Single pass parsing to maintain order
            while !remaining.is_empty() {
                // Find next tag of interest
                let img_pos = remaining.find("<img");
                let attach_pos = remaining.find("<attachment");
                let emoji_pos = remaining.find("<emoji");
                
                // Find the earliest tag
                let next_tag = [
                    img_pos.map(|p| (p, "img")),
                    attach_pos.map(|p| (p, "attachment")),
                    emoji_pos.map(|p| (p, "emoji"))
                ].iter().filter_map(|&x| x).min_by_key(|&(p, _)| p);
                
                if let Some((pos, tag_type)) = next_tag {
                    // Add text before the tag
                    if pos > 0 {
                        segments.push(MessageSegment::Text(remaining[..pos].to_string()));
                    }
                    
                    // Process the tag
                    let tag_start = pos;
                    remaining = &remaining[tag_start..];
                    
                    if let Some(tag_end) = remaining.find('>') {
                        let tag_str = &remaining[..tag_end + 1];
                        
                        match tag_type {
                            "img" => {
                                // Extract src and alt
                                let mut src = None;
                                let mut alt = None;
                                
                                for attr_pattern in &["src=\"", "src='"] {
                                    if let Some(src_start) = tag_str.find(attr_pattern) {
                                        let value_start = src_start + attr_pattern.len();
                                        let quote_char = if attr_pattern.ends_with('"') { '"' } else { '\'' };
                                        if let Some(src_end) = tag_str[value_start..].find(quote_char) {
                                            src = Some(tag_str[value_start..value_start + src_end].to_string());
                                            break;
                                        }
                                    }
                                }
                                
                                for attr_pattern in &["alt=\"", "alt='"] {
                                    if let Some(alt_start) = tag_str.find(attr_pattern) {
                                        let value_start = alt_start + attr_pattern.len();
                                        let quote_char = if attr_pattern.ends_with('"') { '"' } else { '\'' };
                                        if let Some(alt_end) = tag_str[value_start..].find(quote_char) {
                                            alt = Some(tag_str[value_start..value_start + alt_end].to_string());
                                            break;
                                        }
                                    }
                                }
                                
                                segments.push(MessageSegment::Image(ImageInfo { src, alt }));
                                remaining = &remaining[tag_end + 1..];
                            },
                            "attachment" => {
                                // Check if message reference
                                let is_message_reference = tag_str.contains("type=\"messageReference\"")
                                    || tag_str.contains("type='messageReference'")
                                    || tag_str.contains("messageReference");
                                
                                if !is_message_reference {
                                    if let Some(id_start) = tag_str.find("id=\"") {
                                        let value_start = id_start + 4;
                                        if let Some(id_end) = tag_str[value_start..].find('"') {
                                            let attachment_id = tag_str[value_start..value_start + id_end].to_string();
                                            segments.push(MessageSegment::Attachment(attachment_id));
                                        }
                                    }
                                }
                                
                                // Handle closing tag
                                if tag_str.ends_with("/>") {
                                    remaining = &remaining[tag_end + 1..];
                                } else {
                                    remaining = &remaining[tag_end + 1..];
                                    if let Some(close_start) = remaining.find("</attachment>") {
                                        remaining = &remaining[close_start + 13..];
                                    }
                                }
                            },
                            "emoji" => {
                                // Extract alt text for emoji
                                if let Some(alt_start) = tag_str.find("alt=\"") {
                                    let value_start = alt_start + 5;
                                    if let Some(alt_end) = tag_str[value_start..].find('"') {
                                        let emoji = tag_str[value_start..value_start + alt_end].to_string();
                                        segments.push(MessageSegment::Text(emoji));
                                    }
                                }
                                
                                // Handle closing tag
                                remaining = &remaining[tag_end + 1..];
                                if remaining.starts_with("</emoji") {
                                    if let Some(close_end) = remaining.find('>') {
                                        remaining = &remaining[close_end + 1..];
                                    }
                                }
                            },
                            _ => {
                                // Should not happen given filter above
                                remaining = &remaining[tag_end + 1..];
                            }
                        }
                    } else {
                        // Malformed tag
                        segments.push(MessageSegment::Text(remaining[..1].to_string()));
                        remaining = &remaining[1..];
                    }
                } else {
                    // No more tags, add remaining text
                    segments.push(MessageSegment::Text(remaining.to_string()));
                    remaining = "";
                }
            }

            // Helper to clean text
            let clean_text = |text: &str| -> String {
                let mut cleaned = text.to_string()
                    .replace("&nbsp;", " ")
                    .replace("&amp;", "&")
                    .replace("&lt;", "<")
                    .replace("&gt;", ">")
                    .replace("&quot;", "\"")
                    .replace("&#39;", "'")
                    .replace("&apos;", "'")
                    .replace("&#160;", " ")
                    .replace("&nbsp", " ");
                
                cleaned = cleaned
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
                
                // Remove other HTML tags
                let mut no_html = String::new();
                let mut inside_tag = false;
                for c in cleaned.chars() {
                    if c == '<' { inside_tag = true; }
                    else if c == '>' { inside_tag = false; }
                    else if !inside_tag { no_html.push(c); }
                }
                
                // Clean whitespace
                let mut final_content = String::new();
                let mut consecutive_newlines = 0;
                for c in no_html.chars() {
                    if c == '\n' {
                        consecutive_newlines += 1;
                        if consecutive_newlines <= 2 { final_content.push(c); }
                    } else {
                        consecutive_newlines = 0;
                        final_content.push(c);
                    }
                }
                final_content.trim().to_string()
            };

            // Header logic (same as before)
            if show_header {
                if !lines.is_empty() {
                    lines.push(Line::from(""));
                }

                let header = if is_me {
                    format!("{} {}", date_str, "Me")
                } else {
                    format!("{} {}", sender_name, date_str)
                };

                if is_me {
                    let padding = width.saturating_sub(header.len());
                    let pad_str = " ".repeat(padding);
                    lines.push(Line::from(vec![
                        Span::raw(pad_str),
                        Span::styled(header, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::styled(header, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    ]));
                }
            }

            // Render segments
            for segment in segments {
                match segment {
                    MessageSegment::Text(text) => {
                        let cleaned = clean_text(&text);
                        if cleaned.is_empty() { continue; }
                        
                        let mut wrapped_lines = Vec::new();
                        for line in cleaned.lines() {
                            let mut current_line = String::new();
                            for word in line.split_whitespace() {
                                if current_line.len() + word.len() + 1 > max_line_width {
                                    wrapped_lines.push(current_line);
                                    current_line = String::from(word);
                                } else {
                                    if !current_line.is_empty() { current_line.push(' '); }
                                    current_line.push_str(word);
                                }
                            }
                            if !current_line.is_empty() { wrapped_lines.push(current_line); }
                        }
                        
                        if is_me {
                            for line in wrapped_lines {
                                let padding = width.saturating_sub(line.len());
                                let pad_str = " ".repeat(padding);
                                lines.push(Line::from(vec![
                                    Span::raw(pad_str),
                                    Span::raw(line),
                                ]));
                            }
                        } else {
                            for line in wrapped_lines {
                                lines.push(Line::from(line));
                            }
                        }
                    },
                    MessageSegment::Image(info) => {
                        let image_name = info.alt.clone()
                            .or_else(|| {
                                info.src.as_ref().and_then(|s| {
                                    s.split('/').last()
                                        .or_else(|| s.split('\\').last())
                                        .map(|n| n.split('?').next().unwrap_or(n).to_string())
                                })
                            })
                            .unwrap_or_else(|| "Pasted Image".to_string());
                        
                        let icon = "🖼️"; // Always use image icon for pasted images
                        let attachment_text = format!("{} {}", icon, image_name);
                        
                        if is_me {
                            let padding = width.saturating_sub(attachment_text.len());
                            let pad_str = " ".repeat(padding);
                            lines.push(Line::from(vec![
                                Span::raw(pad_str),
                                Span::styled(attachment_text, Style::default().fg(Color::Yellow)),
                            ]));
                        } else {
                            lines.push(Line::from(vec![
                                Span::styled(attachment_text, Style::default().fg(Color::Yellow)),
                            ]));
                        }
                    },
                    MessageSegment::Attachment(id) => {
                        // Find attachment details in API response
                        let mut name = None;
                        let mut content_type = None;
                        
                        if let Some(api_attachments) = &msg.attachments {
                            if let Some(api_att) = api_attachments.iter().find(|a| a.id == id) {
                                name = api_att.name.clone();
                                content_type = api_att.content_type.clone();
                            }
                        }
                        
                        let attachment_name = name.unwrap_or_else(|| "Attachment".to_string());
                        let icon = get_attachment_icon(&attachment_name, content_type.as_deref());
                        let attachment_text = format!("{} {}", icon, attachment_name);
                        
                        if is_me {
                            let padding = width.saturating_sub(attachment_text.len());
                            let pad_str = " ".repeat(padding);
                            lines.push(Line::from(vec![
                                Span::raw(pad_str),
                                Span::styled(attachment_text, Style::default().fg(Color::Yellow)),
                            ]));
                        } else {
                            lines.push(Line::from(vec![
                                Span::styled(attachment_text, Style::default().fg(Color::Yellow)),
                            ]));
                        }
                    }
                }
            }
            
            // If no segments produced lines (empty message), add empty line
            if lines.is_empty() || (show_header && lines.len() == 1) { // 1 for header
                 // Check if we actually added any content lines for this message
                 // This is a bit rough, but ensures we don't have invisible messages
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
                .title(if app.input_mode { "Messages (ESC to cancel)" } else { "Messages (i to compose, PgUp(K)/PgDn(J) to scroll)" })
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
                    .title("Type your message (Enter to send, Alt+Enter for new line, ESC to cancel)")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green))
            )
            .style(Style::default().fg(Color::White))
            .wrap(ratatui::widgets::Wrap { trim: false });
        
        f.render_widget(input_widget, messages_chunks[1]);
        
        // Calculate cursor position for multiline text
        let lines: Vec<&str> = app.input_buffer.split('\n').collect();
        let line_count = lines.len();
        let current_line = if line_count > 0 { line_count - 1 } else { 0 };
        let current_line_text = lines.get(current_line).unwrap_or(&"");
        let cursor_x = messages_chunks[1].x + current_line_text.len() as u16 + 1;
        let cursor_y = messages_chunks[1].y + current_line as u16 + 1;
        
        // Make sure cursor doesn't go beyond the input area
        let max_y = messages_chunks[1].y + messages_chunks[1].height.saturating_sub(1);
        let final_y = std::cmp::min(cursor_y, max_y);
        
        f.set_cursor_position((cursor_x, final_y));
    }

    // Status bar
    let status_border_style = if is_flashing {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD) // Flash Red
    } else {
        Style::default().fg(Color::Green)
    };
    
    let status_text_style = if is_flashing {
        Style::default().fg(Color::Red).bg(Color::White).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Green)
    };

    let status = Paragraph::new(format!("{} | Notification (n): {}", app.status, app.notification_mode))
        .block(
            Block::default()
                .title("Status")
                .borders(Borders::ALL)
                .border_style(status_border_style)
        )
        .style(status_text_style);

    f.render_widget(status, main_chunks[1]);
}
