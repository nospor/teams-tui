use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use crate::app::App;
use ratatui_image::{StatefulImage, Resize};

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
    struct ImageToRender {
        id: String,
        line_index: usize,
        width: u16,
        height: u16,
        is_me: bool,
    }
    let mut images_to_render: Vec<ImageToRender> = Vec::new();

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
            
            
            // Strip HTML tags and extract text content
            let mut clean_content = content.to_string();
            
            // Extract pasted images (img tags) before processing attachments
            #[derive(Clone)]
            struct ImageInfo {
                src: Option<String>,
                alt: Option<String>,
            }
            
            let mut pasted_images = Vec::new();
            let mut img_processed = String::new();
            let mut remaining = clean_content.as_str();
            
            while let Some(img_start) = remaining.find("<img") {
                // Add text before the img tag
                img_processed.push_str(&remaining[..img_start]);
                
                // Find the end of the img tag
                if let Some(tag_end) = remaining[img_start..].find('>') {
                    let tag_str = &remaining[img_start..img_start + tag_end + 1];
                    
                    // Extract src and alt attributes
                    let mut src = None;
                    let mut alt = None;
                    
                    // Try to find src attribute
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
                    
                    // Try to find alt attribute
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
                    
                    // If we found an image (has src or alt), store it
                    if src.is_some() || alt.is_some() {
                        pasted_images.push(ImageInfo { src, alt });
                    }
                    
                    // Skip past the img tag
                    remaining = &remaining[img_start + tag_end + 1..];
                } else {
                    // Malformed tag, skip the <img part
                    img_processed.push_str(&remaining[..img_start + 4]);
                    remaining = &remaining[img_start + 4..];
                }
            }
            
            // Add remaining text
            img_processed.push_str(remaining);
            clean_content = img_processed;
            
            // Extract attachment IDs from HTML and match with API attachments
            // Skip message reference attachments (quoted/replied messages)
            let mut attachment_ids = Vec::new();
            let mut attachment_removed = String::new();
            remaining = clean_content.as_str();
            
            while let Some(attach_start) = remaining.find("<attachment") {
                // Add text before the attachment tag
                attachment_removed.push_str(&remaining[..attach_start]);
                
                // Find the end of the opening tag
                if let Some(tag_end) = remaining[attach_start..].find('>') {
                    let tag_str = &remaining[attach_start..attach_start + tag_end + 1];
                    
                    // Check if this is a message reference attachment
                    // Message references often have type="messageReference" or similar
                    let is_message_reference = tag_str.contains("type=\"messageReference\"")
                        || tag_str.contains("type='messageReference'")
                        || tag_str.contains("messageReference");
                    
                    // Extract attachment ID from the tag (only if not a message reference)
                    if !is_message_reference {
                        if let Some(id_start) = tag_str.find("id=\"") {
                            let value_start = id_start + 4; // len("id=\"")
                            if let Some(id_end) = tag_str[value_start..].find('"') {
                                let attachment_id = tag_str[value_start..value_start + id_end].to_string();
                                attachment_ids.push(attachment_id);
                            }
                        }
                    }
                    
                    // Check if it's self-closing (ends with />)
                    if tag_str.ends_with("/>") {
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
            
            // Store attachment info for display
            // Since $expand=attachments may not be supported, we'll use IDs from HTML
            // and try to match with API attachments if available
            #[derive(Clone)]
            struct AttachmentDisplay {
                id: String,
                name: Option<String>,
                content_type: Option<String>,
            }
            
            let mut attachments = Vec::new();
            let mut message_reference_ids = std::collections::HashSet::new();
            
            if let Some(api_attachments) = &msg.attachments {
                // Match with API attachments if available
                for attachment_id in &attachment_ids {
                    if let Some(api_attachment) = api_attachments.iter().find(|a| a.id == *attachment_id) {
                        // Skip message reference attachments (quoted/replied messages)
                        // These have specific content types like "messageReference" or "application/vnd.microsoft.teams.message"
                        let is_message_reference = api_attachment.content_type.as_ref()
                            .map(|ct| {
                                let ct_lower = ct.to_lowercase();
                                ct_lower.contains("messagereference") 
                                    || ct_lower.contains("vnd.microsoft.teams.message")
                                    || ct_lower == "message/reference"
                            })
                            .unwrap_or(false);
                        
                        if is_message_reference {
                            // Mark this ID as a message reference so we don't add it from HTML fallback
                            message_reference_ids.insert(attachment_id.clone());
                        } else {
                            // Only add if it's not a message reference
                            attachments.push(AttachmentDisplay {
                                id: api_attachment.id.clone(),
                                name: api_attachment.name.clone(),
                                content_type: api_attachment.content_type.clone(),
                            });
                        }
                    }
                }
            }
            
            // If no API attachments matched, use IDs from HTML
            // But skip message reference IDs that we identified from the API
            for attachment_id in &attachment_ids {
                if !attachments.iter().any(|a| a.id == *attachment_id) {
                    // Skip if this is a known message reference
                    if !message_reference_ids.contains(attachment_id) {
                        attachments.push(AttachmentDisplay {
                            id: attachment_id.clone(),
                            name: None,
                            content_type: None,
                        });
                    }
                }
            }
            

            
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
            
            // Add pasted images as attachments
            for image in &pasted_images {
                // Use alt text as name if available, otherwise use src filename or "Pasted Image"
                let image_name = image.alt.clone()
                    .or_else(|| {
                        image.src.as_ref().and_then(|s| {
                            // Try to extract filename from URL
                            s.split('/').last()
                                .or_else(|| s.split('\\').last())
                                .map(|n| n.split('?').next().unwrap_or(n).to_string())
                        })
                    })
                    .unwrap_or_else(|| "Pasted Image".to_string());
                
                // Check if we have this image in cache
                if let Some(src) = &image.src {
                    if app.load_images && app.image_cache.contains_key(src) {
                        // It's an image we can render!
                        // We'll add it to our render list instead of just showing text
                        // But we also want to show the text "Pasted Image" maybe?
                        // For now, let's just render the image.
                        
                        // Calculate dimensions
                        // Heuristic: 1 col ~= 10px width, 1 row ~= 20px height (1:2 aspect ratio of cell)
                        // 80% of width, capped at view width
                        
                        let (img_w, img_h) = if let Some(img) = app.image_cache.get(src) {
                            (img.width(), img.height())
                        } else {
                            (100, 100) // Fallback
                        };
                        
                        let avail_width = width as u16;
                        
                        // Calculate target width in columns
                        // We scale the image pixels to columns (divide by 10)
                        // Then we apply the 80% scaling if it's large, or just ensure it fits
                        // Let's try: target = min(img_w / 8, avail_width * 0.8)
                        // This allows large images to take 80% of screen, and small images to be roughly natural size
                        let target_width = std::cmp::min(
                            (img_w as f32 / 8.0) as u16,
                            (avail_width as f32 * 0.8) as u16
                        );
                        
                        // Ensure at least some width
                        let final_width = std::cmp::max(10, target_width);
                        
                        // Calculate height to maintain aspect ratio
                        // Aspect ratio = w / h
                        // Cell aspect ratio ~= 0.5 (w/h)
                        // rows = cols * (img_h / img_w) / 0.5 = cols * (img_h / img_w) * 2
                        let final_height = (final_width as f32 * (img_h as f32 / img_w as f32) / 2.1) as u16;
                        let final_height = std::cmp::max(1, final_height);

                        images_to_render.push(ImageToRender {
                            id: src.clone(),
                            line_index: lines.len(),
                            width: final_width,
                            height: final_height,
                            is_me,
                        });
                        
                        for _ in 0..final_height {
                            lines.push(Line::from(""));
                        }
                    } else {
                        // Fallback to text attachment if not loaded yet
                        attachments.push(AttachmentDisplay {
                            id: format!("img_{}", attachments.len()), // Unique ID for display
                            name: Some(image_name),
                            content_type: Some("image/png".to_string()), // Default to image, could be enhanced
                        });
                    }
                } else {
                     attachments.push(AttachmentDisplay {
                        id: format!("img_{}", attachments.len()), // Unique ID for display
                        name: Some(image_name),
                        content_type: Some("image/png".to_string()), // Default to image, could be enhanced
                    });
                }
            }
            
            // Display attachments if any
            if !attachments.is_empty() {
                for attachment in &attachments {
                    let attachment_name = attachment.name.as_deref().unwrap_or(&attachment.id);
                    let icon = get_attachment_icon(attachment_name, attachment.content_type.as_deref());
                    let attachment_text = format!("{} {}", icon, attachment_name);
                    
                    if is_me {
                        // Right aligned attachment
                        let padding = width.saturating_sub(attachment_text.len());
                        let pad_str = " ".repeat(padding);
                        lines.push(Line::from(vec![
                            Span::raw(pad_str),
                            Span::styled(attachment_text, Style::default().fg(Color::Yellow)),
                        ]));
                    } else {
                        // Left aligned attachment
                        lines.push(Line::from(vec![
                            Span::styled(attachment_text, Style::default().fg(Color::Yellow)),
                        ]));
                    }
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

    // Render images over the paragraph
    let inner_area = messages_chunks[0].inner(ratatui::layout::Margin { vertical: 1, horizontal: 1 });
    
    for img_info in images_to_render {
        // Calculate visual position
        // line_index is 0-based index in lines
        // scroll_offset is how many lines are scrolled off the top
        
        // If the image line is before the scroll offset, it's not visible (or partially)
        // If it's after scroll_offset + height, it's not visible
        
        let line_y = img_info.line_index as i32 - app.scroll_offset as i32;
        
        // Check visibility
        if line_y + (img_info.height as i32) > 0 && line_y < inner_area.height as i32 {
            // Calculate intersection with viewport
            let render_y = std::cmp::max(0, line_y);
            let skip_lines = if line_y < 0 { -line_y } else { 0 };
            let render_height = std::cmp::min(img_info.height as i32 - skip_lines as i32, inner_area.height as i32 - render_y);
            
            if render_height > 0 {
                if let Some(image) = app.image_cache.get(&img_info.id) {
                    // Get or create protocol
                    if !app.image_protocols.contains_key(&img_info.id) {
                        let protocol = app.picker.new_resize_protocol(image.clone());
                        app.image_protocols.insert(img_info.id.clone(), protocol);
                    }
                    
                    if let Some(protocol) = app.image_protocols.get_mut(&img_info.id) {
                        let widget = StatefulImage::new(None).resize(Resize::Fit(None));
                        
                        let x = if img_info.is_me {
                            // Right aligned
                            inner_area.width.saturating_sub(img_info.width)
                        } else {
                            0
                        };
                        
                        let area = ratatui::layout::Rect {
                            x: inner_area.x + x,
                            y: inner_area.y + render_y as u16,
                            width: img_info.width,
                            height: render_height as u16,
                        };
                        
                        f.render_stateful_widget(widget, area, protocol);
                    }
                }
            }
        }
    }

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
    let status = Paragraph::new(app.status.as_str())
        .block(
            Block::default()
                .title("Status")
                .borders(Borders::ALL)
        )
        .style(Style::default().fg(Color::Green));

    f.render_widget(status, main_chunks[1]);
}
