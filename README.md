# Teams Terminal Client

A Rust-based Terminal User Interface (TUI) for Microsoft Teams.

## Features

- 🔐 Secure OAuth2 authentication (Device Code Flow)
- 💬 View your Teams chats
- ⌨️ Keyboard navigation (Vim-style or arrow keys)
- 🎨 Modern, colorful terminal UI
- 💾 Token persistence (no need to re-authenticate)

## Quick Start

**Important:** You need to register your own Azure AD application first!

### 1. Register Azure AD App

Follow the detailed guide in [AZURE_SETUP.md](AZURE_SETUP.md) to:
- Register an app in Azure Portal
- Get your Client ID
- Configure permissions

### 2. Configure the App

Create a `.env` file in this directory:
```bash
cp .env.example .env
nano .env
```

Replace `your-client-id-here` with your actual Client ID from Azure.

### 3. Run the Application

```bash
# Run the application
cargo run

# Or build and run the binary
cargo build --release
./target/release/teams-terminal
```

## First Time Setup

1. Run the app
2. Open the displayed URL in your browser
3. Enter the code shown in the terminal
4. Sign in with your Microsoft account
5. Grant permissions

## Keyboard Controls

- `↑` / `k` - Move up
- `↓` / `j` - Move down  
- `q` - Quit

## Requirements

- Rust 1.70+ (2021 edition)
- Microsoft account with Teams access

## How It Works

This app uses:
- **Microsoft Graph API** to fetch Teams data
- **Ratatui** for the terminal UI
- **OAuth2 Device Code Flow** for authentication

Tokens are saved to `~/.config/teams-terminal/token.json` and automatically refreshed.

## License

MIT
