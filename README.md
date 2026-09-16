# 🗂️ Roslin

**Natural-language file management agent for Windows.** Talk to your files. Organize, find, and manage your documents through conversational AI.

---

## Overview

Roslin is a desktop agent that brings conversational AI to file management. Instead of dragging, clicking, and navigating folder trees, you tell Roslin what you want in plain English — and it handles the work.

**Example interactions:**
- *"Consolidate the Stanley merger files into a folder called Stanley_Merger and zip it"*
- *"Find my Spotify resume in Downloads"*
- *"Delete files older than 5 years to free up space"*

---

## Features

### 🔍 **File Discovery**
Search for files and folders by name across any directory. Supports multi-keyword matching across recursive subdirectories.

### 📁 **File Organization**
Move, consolidate, and reorganize files into folders with a single conversational prompt. Create new folders on demand.

### 📦 **Compression**
Zip files and folders into archives without leaving the chat interface.

### 📝 **File Creation**
Create text-based files (`.txt`, `.py`, `.md`, `.json`, `.csv`, etc.) with content from your prompts.

### 👁️ **File Reading**
Read and inspect file contents (up to 2000 chars) to help Claude understand what you're looking for or working with.

### 🗑️ **Safe Deletion**
Delete files with confidence. Roslin stages deletions first, shows you exactly what will be removed, and requires explicit confirmation before anything is permanently deleted. Full audit trail included.

### 💾 **Conversation Persistence**
Your chat history is automatically saved and restored when you close and reopen Roslin. Pick up where you left off.

---

## Tech Stack

- **Frontend:** Vanilla JavaScript + HTML/CSS
- **Backend:** Rust (Tauri) + reqwest
- **AI Engine:** Claude API (claude-sonnet-4-6)
- **Storage:** Local JSON + OS Keychain (for API keys)
- **File Operations:** Rust `walkdir`, `zip`, `fs` crates

---

## Getting Started

### Prerequisites
- Windows 10 or later
- Node.js 18+
- Rust (via rustup)
- Anthropic API key ([get one here](https://console.anthropic.com))

### Installation

1. Clone the repo:
```bash
   git clone https://github.com/YOUR_USERNAME/roslin.git
   cd roslin
```

2. Install dependencies:
```bash
   npm install
   cd src-tauri && cargo build && cd ..
```

3. Set your API key:
```bash
   # Create src-tauri/.env
   echo ANTHROPIC_API_KEY=sk-ant-... > src-tauri/.env
```

4. Run in dev mode:
```bash
   npm run tauri dev
```

---

## Usage

1. **Open Roslin** — a tray icon appears on your taskbar
2. **Type a prompt** — describe what you want to do with your files
3. **Review and confirm** — for destructive actions (deletions), you'll see exactly what will be affected and must confirm
4. **Done** — Roslin executes and reports back

All conversations are saved automatically. Clear history anytime with the "Clear conversation" button.

---

## Current Limitations

- **Windows only** — macOS and Linux support coming later
- **Text files only** — can create `.txt`, `.py`, `.md` etc. but not `.docx`, `.pdf` (those need specialized libraries)
- **File reading cap** — reads up to 2000 characters per file
- **Search results capped** — returns max 50 results per list to avoid token bloat

---

## Safety & Privacy

- **API key stored securely** — kept in Windows Credential Manager, never sent in chat
- **Local-first** — all file operations happen on your machine; only file metadata and your prompts reach Claude
- **Reversible deletions** — nothing is deleted without your explicit confirmation
- **Audit trail** — every action is logged and visible

---

## Contributing

Found a bug? Have a feature idea? Open an issue or PR.

---

## License

MIT

---

**Built by [Kamsi](https://github.com/kamC0des) • Powered by [Claude](https://anthropic.com)**