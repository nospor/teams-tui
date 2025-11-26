# Azure AD App Registration Guide

## Why You Need This

The Microsoft Graph Explorer Client ID has restrictions. You need to register your own Azure AD application to authenticate with Microsoft Teams.

## Step-by-Step Instructions

### 1. Go to Azure Portal

Open your browser and navigate to:
**https://entra.microsoft.com/** (or **https://portal.azure.com**)

### 2. Register a New Application

1. In the left sidebar, navigate to: **Identity** → **Applications** → **App registrations**
2. Click **+ New registration**
3. Fill in the form:
   - **Name**: `Teams TUI Client` (or any name you prefer)
   - **Supported account types**: Select **"Accounts in any organizational directory and personal Microsoft accounts"**
   - **Redirect URI**: Leave this **blank** (device code flow doesn't need it)
4. Click **Register**

### 3. Note Your Application (Client) ID

After registration, you'll see the **Overview** page:
- Copy the **Application (client) ID** (it looks like: `12345678-1234-1234-1234-123456789abc`)
- **Save this ID** - you'll need it in the next step!

### 4. Enable Public Client Flow

1. In the left sidebar, click **Authentication**
2. Scroll down to **Advanced settings**
3. Find **"Allow public client flows"**
4. Toggle it to **Yes**
5. Click **Save** at the top

### 5. Add API Permissions

1. In the left sidebar, click **API permissions**
2. Click **+ Add a permission**
3. Select **Microsoft Graph**
4. Select **Delegated permissions**
5. **IMPORTANT**: Search for and add these **exact** permissions:
   - Type `User.Read` and check the box
   - Type `Chat.Read` and check the box
   - Type `Chat.ReadWrite` and check the box  
   - Type `offline_access` and check the box
   
   > **Note**: Make sure you're adding **Delegated** permissions, not Application permissions!
   
6. Click **Add permissions**
7. *Optional*: Click **Grant admin consent for [Your Organization]** (if you're an admin)

**After adding permissions, your API permissions list should show:**
- Microsoft Graph (4):
  - User.Read
  - Chat.Read
  - Chat.ReadWrite
  - offline_access

### Troubleshooting: "Invalid Scope" Error

If you get an error like `The scope 'Chat.Read Chat.ReadWrite offline_access' does not exist`, it means:

1. **You haven't added the permissions yet** - Go back to step 5 and add them
2. **You added Application permissions instead of Delegated** - Remove them and add Delegated permissions
3. **The permissions weren't saved** - Refresh the Azure Portal page and check if they're still there

### Troubleshooting: "Failed to get current user info"

If you get this error, you're missing the `User.Read` permission:

1. Go to **API permissions** in Azure Portal
2. Click **+ Add a permission** → **Microsoft Graph** → **Delegated permissions**
3. Search for and add `User.Read`
4. **Delete your saved token**: `rm ~/.config/teams-tui/token.json`
5. Run the app again and re-authenticate

### 6. Update Your Application

See the **README.md** for instructions on setting the Client ID (Quick Start, point 2).

### 7. Run the Application

```bash
cargo run
```

Now the authentication should work!

## Troubleshooting

**Q: I don't have access to Azure Portal**  
A: Ask your IT administrator to register the app for you, or use a personal Microsoft account.

**Q: I get "Admin consent required"**  
A: Some organizations require admin approval. Contact your IT department.

**Q: Where do I find the Azure Portal?**  
A: Go to https://entra.microsoft.com/ or https://portal.azure.com/

## What Permissions Mean

- **Chat.Read**: Read your Teams chat messages
- **Chat.ReadWrite**: Read and send Teams chat messages
- **offline_access**: Keep you logged in (refresh tokens)
