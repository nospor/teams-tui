mod app;
mod ui;
mod auth;
mod api;

use std::io;
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use crate::app::App;

#[tokio::main]
async fn main() -> Result<()> {
    // Authenticate first (before setting up terminal)
    println!("Microsoft Teams Terminal Client");
    println!("================================\n");
    
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

    // Fetch chats
    println!("Fetching chats...");
    let chats = match api::get_chats(&access_token).await {
        Ok(chats) => {
            println!("✓ Loaded {} chats\n", chats.len());
            chats
        }
        Err(e) => {
            eprintln!("✗ Failed to fetch chats: {}", e);
            return Err(e);
        }
    };

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new();
    app.set_chats(chats);

    // Run app
    let res = run_app(&mut terminal, &mut app).await;

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

async fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Down | KeyCode::Char('j') => app.next_chat(),
                KeyCode::Up | KeyCode::Char('k') => app.previous_chat(),
                _ => {}
            }
        }
    }
}
