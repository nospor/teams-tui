mod app;
mod ui;
mod auth;
mod api;
pub mod config;

use std::io;
use std::collections::{HashMap, HashSet};
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use image::ImageReader;
use std::io::Cursor;
use image::DynamicImage;
use crate::app::App;

#[tokio::main]
async fn main() -> Result<()> {
    // Authenticate first (before setting up terminal)
    println!("TeamsTUI");
    println!("================================\n");
    
    // Load configuration
    let config = config::load_config().unwrap_or(config::Config {
        client_id: None,
        tenant_id: None,
        load_images: false, // Default to false as requested
    });

    let access_token = match auth::get_access_token().await {
        Ok(token) => {
            println!("✓ Authentication successful!\n");
            token
        }
        Err(e) => {
            eprintln!("✗ Authentication failed: {}", e);
            return Err(e);
        }
    };

    // Fetch current user profile
    println!("Fetching user profile...");
    let current_user = match api::get_me(&access_token).await {
        Ok(user) => {
            println!("✓ Logged in as: {}\n", user.display_name);
            Some(user)
        }
        Err(e) => {
            eprintln!("⚠ Failed to fetch user profile: {}", e);
            None
        }
    };

    // Fetch chats
    println!("Fetching chats...");
    let (mut chats, _) = match api::get_chats(&access_token).await {
        Ok(result) => {
            println!("✓ Loaded {} chats\n", result.0.len());
            result
        }
        Err(e) => {
            eprintln!("✗ Failed to fetch chats: {}", e);
            return Err(e);
        }
    };
    
    // Load last message for each chat in parallel to get accurate sorting by actual message time
    println!("Loading last messages for accurate sorting...");
    let mut chat_last_message_times: HashMap<String, String> = HashMap::new();
    
    // Spawn tasks to load messages for all chats in parallel
    let mut tasks = Vec::new();
    for chat in &chats {
        let chat_id = chat.id.clone();
        let token = access_token.clone();
        tasks.push(tokio::spawn(async move {
        if let Ok(messages) = api::get_messages(&token, &chat_id).await {
            if messages.is_empty() {
                None
            } else {
                // Messages are returned in chronological order (oldest first) based on UI code using .rev()
                // So the last message in the array is the most recent
                // But let's be safe and compare first vs last to get the most recent
                let first = messages.first().unwrap();
                let last = messages.last().unwrap();
                
                // Use whichever is more recent (later timestamp)
                let most_recent = if last.created_date_time > first.created_date_time {
                    last
                } else {
                    first
                };
                
                Some((chat_id, most_recent.created_date_time.clone()))
            }
        } else {
            None
        }
        }));
    }
    
    // Wait for all tasks to complete
    let mut loaded_count = 0;
    for task in tasks {
        if let Ok(Some((chat_id, time))) = task.await {
            chat_last_message_times.insert(chat_id, time);
            loaded_count += 1;
        }
    }
    
    println!("Loaded messages for {}/{} chats", loaded_count, chats.len());
    
    // Sort chats by actual last message time (most recent first)
    // Chats with messages come first, sorted by message time
    // Chats without messages come after, sorted by lastUpdatedDateTime
    chats.sort_by(|a, b| {
        let a_time = chat_last_message_times.get(&a.id);
        let b_time = chat_last_message_times.get(&b.id);
        
        match (a_time, b_time) {
            (Some(a_t), Some(b_t)) => {
                // Both have messages - compare timestamps (most recent first)
                // Timestamps are in ISO 8601 format, so string comparison works
                b_t.cmp(a_t)
            },
            (Some(_), None) => std::cmp::Ordering::Less, // a has messages, b doesn't - a comes first
            (None, Some(_)) => std::cmp::Ordering::Greater, // b has messages, a doesn't - b comes first
            (None, None) => {
                // Neither has messages, use lastUpdatedDateTime as fallback
                match (&a.last_updated, &b.last_updated) {
                    (Some(a_dt), Some(b_dt)) => b_dt.cmp(a_dt), // Most recent first
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            }
        }
    });
    
    println!("✓ Sorted {} chats by last message time\n", chats.len());

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new(config.load_images);
    app.set_chats(chats);
    if let Some(user) = current_user {
        app.set_current_user(user.display_name);
    }

    // Run app (pass the initial message times we loaded and access token for background tasks)
    let res = run_app(&mut terminal, &mut app, chat_last_message_times, access_token).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

async fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App, mut chat_last_message_times: HashMap<String, String>, access_token: String) -> Result<()> {
    // Create a channel for receiving loaded messages
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(usize, Vec<api::Message>)>();
    
    // Create a channel for receiving chat updates
    let (tx_chats, mut rx_chats) = tokio::sync::mpsc::unbounded_channel::<(Vec<api::Chat>, Option<String>)>();
    
    // Create a channel for new message notifications (chat_id, last_message)
    let (tx_new_messages, mut rx_new_messages) = tokio::sync::mpsc::unbounded_channel::<(String, api::Message)>();

    // Create a channel for loaded images
    let (tx_images, mut rx_images) = tokio::sync::mpsc::unbounded_channel::<(String, DynamicImage)>();

    
    // Store the latest chat list from API for use when reordering
    let mut latest_chats_from_api: Option<Vec<api::Chat>> = None;
    
    // Track last message ID per chat to detect actual new messages
    // Maps chat_id -> last_message_id
    let mut chat_last_message_ids: HashMap<String, String> = HashMap::new();
    
    // chat_last_message_times is passed in from initial load
    // stable_chat_order is initialized from the correctly sorted initial load
    
    // Store the stable order of chats (by chat ID) to prevent reordering
    // This is initialized from the correctly sorted initial load
    // We preserve this order on refresh and only reorder when new messages are detected
    let mut stable_chat_order: Vec<String> = app.chats.iter().map(|c| c.id.clone()).collect();

    // Track pending image fetches to avoid duplicate requests
    let mut pending_images: HashSet<String> = HashSet::new();

    // Spawn background task to refresh chats
    let tx_chats_clone = tx_chats.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if let Ok(token) = auth::get_valid_token_silent().await {
                if let Ok(result) = api::get_chats(&token).await {
                    let _ = tx_chats_clone.send(result);
                }
            }
        }
    });
    
    let mut last_selected_chat_id: Option<String> = None;
    let mut message_refresh_counter = 0u32;
    
    // Load messages for the first chat if available
    if let Some(chat) = app.get_selected_chat() {
        let chat_id = chat.id.clone();
        let chat_index = app.selected_index;
        let tx_clone = tx.clone();
        
        app.set_loading_messages(true);
        tokio::spawn(async move {
            if let Ok(token) = auth::get_valid_token_silent().await {
                if let Ok(messages) = api::get_messages(&token, &chat_id).await {
                    let _ = tx_clone.send((chat_index, messages));
                }
            }
        });
    }
    
    loop {
        // Check for chat updates
        while let Ok((chats_from_api, _)) = rx_chats.try_recv() {
            // Store latest chats for use when reordering after new message detection
            latest_chats_from_api = Some(chats_from_api.clone());
            
            // Preserve selection by chat ID (not index, since order may change)
            let old_index = app.selected_index;
            let current_chat_id = if old_index < app.chats.len() {
                app.chats.get(old_index).map(|c| c.id.clone())
            } else {
                None
            };
            
            // Check ALL chats for new messages by loading their last message
            // We need to check all chats because lastUpdatedDateTime can change when marked as read,
            // so we can't rely on it alone. Instead, we'll load the last message and compare IDs.
            let current_selected_chat_id = app.get_selected_chat().map(|c| c.id.clone());
            
            for chat in &chats_from_api {
                // Skip the currently selected chat - it's handled separately
                if Some(&chat.id) == current_selected_chat_id.as_ref() {
                    continue;
                }
                
                let tx_new_clone = tx_new_messages.clone();
                let chat_id_clone = chat.id.clone();
                
                // Load last message for this chat to check if there's a new one
                tokio::spawn(async move {
                    if let Ok(token) = auth::get_valid_token_silent().await {
                        if let Ok(messages) = api::get_messages(&token, &chat_id_clone).await {
                            if let Some(last_msg) = messages.last() {
                                // Send the last message - the main loop will check if it's new
                                let _ = tx_new_clone.send((chat_id_clone, last_msg.clone()));
                            }
                        }
                    }
                });
            }
            
            // Add any new chats (chats not in stable order) to stable_chat_order
            // If they already have message times (detected as new), add them at the top
            // Otherwise, add them at the end
            for chat in &chats_from_api {
                if !stable_chat_order.contains(&chat.id) {
                    if chat_last_message_times.contains_key(&chat.id) {
                        // This chat has messages (detected as new), add to top
                        stable_chat_order.insert(0, chat.id.clone());
                    } else {
                        // New chat without messages yet, add to end
                        stable_chat_order.push(chat.id.clone());
                    }
                }
            }
            
            // Rebuild chat list based on stable_chat_order (which has new chats with messages at top)
            let mut reordered_chats = Vec::new();
            for chat_id in &stable_chat_order {
                if let Some(chat) = chats_from_api.iter().find(|c| &c.id == chat_id) {
                    reordered_chats.push(chat.clone());
                }
            }
            
            app.set_chats(reordered_chats);
            
            // Restore selection by finding the chat by ID
            if let Some(id) = current_chat_id {
                if let Some(new_index) = app.chats.iter().position(|c| c.id == id) {
                    app.selected_index = new_index;
                } else {
                    if app.selected_index >= app.chats.len() {
                        app.selected_index = app.chats.len().saturating_sub(1);
                    }
                }
            } else {
                if app.chats.is_empty() {
                    app.selected_index = 0;
                } else if app.selected_index >= app.chats.len() {
                    app.selected_index = app.chats.len().saturating_sub(1);
                }
            }
            
            // Refresh messages for the currently selected chat to get new messages
            if let Some(chat) = app.get_selected_chat() {
                let chat_id = chat.id.clone();
                let chat_index = app.selected_index;
                let tx_clone = tx.clone();
                
                tokio::spawn(async move {
                    if let Ok(token) = auth::get_valid_token_silent().await {
                        if let Ok(messages) = api::get_messages(&token, &chat_id).await {
                            let _ = tx_clone.send((chat_index, messages));
                        }
                    }
                });
            }
        }
        
        // Periodically refresh messages for the currently selected chat (every ~3 seconds)
        message_refresh_counter += 1;
        if message_refresh_counter >= 30 { // 30 * 100ms = 3 seconds
            message_refresh_counter = 0;
            if let Some(chat) = app.get_selected_chat() {
                let current_chat_id = chat.id.clone();
                // Only refresh if we're still on the same chat (not switching)
                if last_selected_chat_id.as_ref() == Some(&current_chat_id) {
                    let chat_id = current_chat_id.clone();
                    let chat_index = app.selected_index;
                    let tx_clone = tx.clone();
                    
                    tokio::spawn(async move {
                        if let Ok(token) = auth::get_valid_token_silent().await {
                            if let Ok(messages) = api::get_messages(&token, &chat_id).await {
                                let _ = tx_clone.send((chat_index, messages));
                            }
                        }
                    });
                }
                last_selected_chat_id = Some(current_chat_id);
            }
        }

        // Check for new messages in other chats (non-blocking)
        while let Ok((chat_id, last_msg)) = rx_new_messages.try_recv() {
            // Check if this is actually a new message (different from last known)
            let is_new_message = if let Some(old_id) = chat_last_message_ids.get(&chat_id) {
                old_id != &last_msg.id
            } else {
                // First time seeing messages - check if chat is already in stable order
                // If it is, it means we loaded it on initial load, so this might be a new message
                // If it's not, it's a brand new chat that should go to top
                !stable_chat_order.contains(&chat_id)
            };
            
            if is_new_message {
                // New message detected! Move chat to top of stable order
                if let Some(pos) = stable_chat_order.iter().position(|id| id == &chat_id) {
                    stable_chat_order.remove(pos);
                }
                stable_chat_order.insert(0, chat_id.clone());
                
                // Update message ID and time
                chat_last_message_ids.insert(chat_id.clone(), last_msg.id.clone());
                chat_last_message_times.insert(chat_id.clone(), last_msg.created_date_time.clone());
                
                // Always trigger a chat list refresh to get latest data, then reorder
                // This ensures the chat list updates immediately when a new message is detected
                let tx_chats_refresh = tx_chats.clone();
                tokio::spawn(async move {
                    if let Ok(token) = auth::get_valid_token_silent().await {
                        if let Ok(result) = api::get_chats(&token).await {
                            let _ = tx_chats_refresh.send(result);
                        }
                    }
                });
                
                // Also reorder immediately with current data (will be updated again when refresh completes)
                let chats_to_use = latest_chats_from_api.as_ref().unwrap_or(&app.chats);
                let mut reordered = Vec::new();
                for chat_id_in_order in &stable_chat_order {
                    if let Some(chat) = chats_to_use.iter().find(|c| &c.id == chat_id_in_order) {
                        reordered.push(chat.clone());
                    }
                }
                // Add any chats not in stable order (shouldn't happen, but just in case)
                for chat in chats_to_use {
                    if !stable_chat_order.contains(&chat.id) {
                        reordered.push(chat.clone());
                    }
                }
                app.set_chats(reordered);
                
                // Preserve selection if it changed
                if let Some(current_chat) = app.get_selected_chat() {
                    let current_id = current_chat.id.clone();
                    if let Some(new_index) = app.chats.iter().position(|c| c.id == current_id) {
                        app.selected_index = new_index;
                    }
                }
            } else {
                // Not a new message, just update stored data
                chat_last_message_ids.insert(chat_id.clone(), last_msg.id.clone());
                chat_last_message_times.insert(chat_id, last_msg.created_date_time.clone());
            }
        }
        
        // Check for loaded messages (non-blocking)
        while let Ok((chat_index, messages)) = rx.try_recv() {
            // Only update if we're still on the same chat
            if chat_index == app.selected_index {
                // Check if messages actually changed to avoid unnecessary snaps/renders
                let should_update = if app.messages.len() != messages.len() {
                    true
                } else {
                    // Check last message ID
                    match (app.messages.last(), messages.last()) {
                        (Some(curr), Some(new)) => curr.id != new.id,
                        (None, None) => false,
                        _ => true,
                    }
                };

                if should_update {
                    let chat_id = app.chats[chat_index].id.clone();
                    
                    // Check if this is a new message (different from last known)
                    if let Some(last_msg) = messages.last() {
                        let is_new = chat_last_message_ids.get(&chat_id)
                            .map(|old_id| old_id != &last_msg.id)
                            .unwrap_or(false); // First time seeing messages = not new (just loading)
                        
                        // Update last message ID and time
                        chat_last_message_ids.insert(chat_id.clone(), last_msg.id.clone());
                        chat_last_message_times.insert(chat_id.clone(), last_msg.created_date_time.clone());
                        
                        // If new message detected, move chat to top of stable order
                        if is_new {
                            if let Some(pos) = stable_chat_order.iter().position(|id| id == &chat_id) {
                                stable_chat_order.remove(pos);
                                stable_chat_order.insert(0, chat_id);
                            }
                        }
                    }
                    
                    app.set_messages(messages.clone());
                    app.snap_to_bottom = true;
                }
                
                // Always mark the chat as read when we're viewing it (regardless of message updates)
                // This ensures chats are marked as read even if messages haven't changed
                if let Some(chat) = app.get_selected_chat() {
                    let chat_id = chat.id.clone();
                    
                    tokio::spawn(async move {
                        if let Ok(token) = auth::get_valid_token_silent().await {
                            let _ = api::mark_chat_as_read(&token, &chat_id).await;
                        }
                    });
                }
            }
        }
        


        // Check for loaded images
        while let Ok((id, image)) = rx_images.try_recv() {
            app.image_cache.insert(id.clone(), image);
            pending_images.remove(&id);
        }

        // Scan for images to fetch
        if app.load_images && !app.loading_messages {
            let messages = app.messages.clone();
            let cache_keys: Vec<String> = app.image_cache.keys().cloned().collect();
            let tx_images = tx_images.clone();
            let access_token = access_token.clone(); // Use the initial token or get a new one
            
            // We need a way to avoid spawning tasks for already fetching images
            // For now, we'll just check if it's in the cache. 
            // A pending set would be better but let's keep it simple for now.
            
            // Scan the visible messages (first 100, which are the newest)
            for msg in messages.iter().take(100) {
                if let Some(body) = &msg.body {
                    if let Some(content) = &body.content {
                        // Simple extraction of src attributes
                        let mut remaining = content.as_str();
                        while let Some(img_start) = remaining.find("<img") {
                            remaining = &remaining[img_start..];
                            if let Some(src_start) = remaining.find("src=\"") {
                                let start = src_start + 5;
                                if let Some(end) = remaining[start..].find('"') {
                                    let url = remaining[start..start + end].to_string();
                                    
                                    // Use URL as ID for now
                                    if !cache_keys.contains(&url) && !pending_images.contains(&url) && url.starts_with("http") {
                                        let tx = tx_images.clone();
                                        let url_clone = url.clone();
                                        let token = access_token.clone();
                                        
                                        // Mark as pending
                                        pending_images.insert(url.clone());
                                        
                                        // Spawn fetch task
                                        tokio::spawn(async move {
                                            // TODO: Use a fresh token if needed
                                            if let Ok(bytes) = api::get_url_bytes(&token, &url_clone).await {
                                                if let Ok(img) = ImageReader::new(Cursor::new(bytes)).with_guessed_format() {
                                                    if let Ok(decoded) = img.decode() {
                                                        let _ = tx.send((url_clone, decoded));
                                                    }
                                                }
                                            }
                                        });
                                    }
                                }
                            }
                            remaining = &remaining[1..];
                        }
                    }
                }
                
                // Also check for file attachments that are images
                // (Removed file attachment image fetching as it requires extra permissions)
            }
        }
        


        terminal.draw(|f| ui::draw(f, app))?;

        // Use poll with timeout to allow checking for messages
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                let previous_index = app.selected_index;
                
                match key.code {
                    KeyCode::Char('q') if !app.input_mode => return Ok(()),
                    KeyCode::Down | KeyCode::Char('j') if !app.input_mode => {
                        app.next_chat();
                        app.clear_image_protocols();
                        app.clear_messages();
                        
                        // Force clear Kitty images if using Kitty protocol
                        if let ratatui_image::picker::ProtocolType::Kitty = app.picker.protocol_type() {
                            let _ = execute!(io::stdout(), crossterm::style::Print("\x1b_Ga=d,d=A\x1b\\"));
                        }
                        


                        if let Some(chat) = app.get_selected_chat() {
                            let chat_id = chat.id.clone();
                            let chat_index = app.selected_index;
                            let tx = tx.clone();
                            let token = access_token.clone();
                            app.set_loading_messages(true);
                            tokio::spawn(async move {
                                if let Ok(messages) = api::get_messages(&token, &chat_id).await {
                                    let _ = tx.send((chat_index, messages));
                                }
                            });
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') if !app.input_mode => {
                        app.previous_chat();
                        app.clear_image_protocols();
                        app.clear_messages();
                        
                        // Force clear Kitty images if using Kitty protocol
                        if let ratatui_image::picker::ProtocolType::Kitty = app.picker.protocol_type() {
                            let _ = execute!(io::stdout(), crossterm::style::Print("\x1b_Ga=d,d=A\x1b\\"));
                        }
                        


                        if let Some(chat) = app.get_selected_chat() {
                            let chat_id = chat.id.clone();
                            let chat_index = app.selected_index;
                            let tx = tx.clone();
                            let token = access_token.clone();
                            app.set_loading_messages(true);
                            tokio::spawn(async move {
                                if let Ok(messages) = api::get_messages(&token, &chat_id).await {
                                    let _ = tx.send((chat_index, messages));
                                }
                            });
                        }
                    }
                    KeyCode::Char('i') if !app.input_mode => {
                        app.input_mode = true;
                        app.input_buffer.clear();
                    }
                    KeyCode::Esc if app.input_mode => {
                        app.input_mode = false;
                        app.input_buffer.clear();
                    }
                    // Handle Alt+Enter for newline (most reliable method)
                    KeyCode::Enter if app.input_mode && key.modifiers.contains(KeyModifiers::ALT) => {
                        // Alt+Enter = new line
                        app.input_buffer.push('\n');
                    }
                    KeyCode::Enter if app.input_mode => {
                        // Check for Shift or Ctrl modifiers (may not work in all terminals)
                        let has_shift = key.modifiers.contains(KeyModifiers::SHIFT);
                        let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                        
                        if has_shift || has_ctrl {
                            // Shift+Enter or Ctrl+Enter = new line
                            app.input_buffer.push('\n');
                        } else {
                            // Enter without modifiers = send message
                            if !app.input_buffer.is_empty() {
                                let message = app.input_buffer.clone();
                                app.input_buffer.clear();
                                app.input_mode = false;
                                
                                // Send message logic
                                if let Some(chat) = app.get_selected_chat() {
                                    let chat_id = chat.id.clone();
                                    let chat_index = app.selected_index;
                                    let tx = tx.clone();
                                    let tx_chats = tx_chats.clone();
                                    let token = access_token.clone();
                                    
                                    tokio::spawn(async move {
                                        if let Ok(_) = api::send_message(&token, &chat_id, &message).await {
                                            // Reload messages
                                            if let Ok(messages) = api::get_messages(&token, &chat_id).await {
                                                let _ = tx.send((chat_index, messages));
                                            }
                                            // Refresh chat list to update last message preview
                                            if let Ok(chats) = api::get_chats(&token).await {
                                                let _ = tx_chats.send(chats);
                                            }
                                        } else {
                                            eprintln!("Failed to send message");
                                        }
                                    });
                                }
                            }
                        }
                    }
                    KeyCode::Backspace if app.input_mode => {
                        app.input_buffer.pop();
                    }
                    KeyCode::Char(c) if app.input_mode => {
                        app.input_buffer.push(c);
                    }
                    KeyCode::PageUp => {
                        app.snap_to_bottom = false;
                        app.scroll_offset = app.scroll_offset.saturating_sub(10);
                    }
                    KeyCode::PageDown => {
                        app.scroll_offset = app.scroll_offset.saturating_add(10);
                        if app.scroll_offset >= app.max_scroll {
                            app.snap_to_bottom = true;
                        }
                    }
                    _ => {}
                }

                // If selection changed, spawn a background task to load messages
                if previous_index != app.selected_index {
                    if let Some(chat) = app.get_selected_chat() {
                        let chat_id = chat.id.clone();
                        let chat_index = app.selected_index;
                        let tx_clone = tx.clone();
                        
                        app.set_loading_messages(true);
                        app.set_messages(Vec::new()); // Clear old messages immediately
                        app.snap_to_bottom = true; // Snap to bottom for new chat
                        
                        tokio::spawn(async move {
                            if let Ok(token) = auth::get_valid_token_silent().await {
                                if let Ok(messages) = api::get_messages(&token, &chat_id).await {
                                    let _ = tx_clone.send((chat_index, messages));
                                }
                            }
                        });
                    }
                }
            }
        }
    }
}
