# Headroom

<p align="center">
  <strong>macOS Menu Bar AI Subscription Usage Tracker</strong><br>
  Built with Rust + GPUI · Lightweight · Native Menu-Bar-Only
</p>

---

## Overview

**Headroom** is a native macOS menu bar app written in Rust using [GPUI](https://gpui.rs). It tracks your live quota usage across AI coding subscriptions (**Claude Code**, **OpenCode Go**, and **Google Antigravity**) directly from the macOS status bar.

It runs as a pure status-bar accessory (no Dock icon), opens on-demand when clicked, and closes automatically when you click away.

---

## Features

- ⚡ **Native Performance**: Built with Rust & GPUI for instant rendering and near-zero memory footprint.
- 🎯 **Menu-Bar-Only Accessory**: Runs quietly in the macOS top bar without cluttering your Dock (`NSApplicationActivationPolicyAccessory`).
- 📊 **Real-Time Quota Tracking**:
  - **Claude Code**: Live session (5-hour window) and weekly quotas parsed via `claude /usage`.
  - **OpenCode Go**: Rolling (5h), weekly (7d), and monthly (30d) plan usage via the official `https://opencode.ai/zen/go/v1/usage` API.
  - **Google Antigravity**: Quotas and reset countdowns fetched directly from Google Cloud Code Assist (`loadCodeAssist` + `retrieveUserQuota`).
- 🔐 **Secure Credential Storage**: Store API keys in the macOS Keychain (`Headroom` service) with automatic fallback discovery.
- 🎨 **Pixel-Perfect Dark UI**: Clean, non-distracting layout matching macOS system dark mode.

---

## UI Examples

### Current Limits View

```text
┌─────────────────────────────────────────────────────────┐
│ CURRENT LIMITS                      synced 5s ago % left│
├─────────────────────────────────────────────────────────┤
│ [C] Claude Code                     ■■■■■■■■░░    32% ▾ │
│     5h session · reset 11:55                            │
├─────────────────────────────────────────────────────────┤
│ [O] OpenCode Go                     ■■■■■■■■■░    78% ▾ │
│     Daily · reset 00:00                                 │
├─────────────────────────────────────────────────────────┤
│ [G] Gemini CLI (Antigravity)        ■░░░░░░░░░     9% ▾ │
│     Daily · reset 15:59                                 │
│     📉 burn rate rising           exhausted by ~15:20   │
├─────────────────────────────────────────────────────────┤
│ Preferences…                             Refresh now ⌘R │
└─────────────────────────────────────────────────────────┘
```

### Preferences Pane

```text
┌─────────────────────────────────────────────────────────┐
│ ‹ Back PREFERENCES                                      │
├─────────────────────────────────────────────────────────┤
│ Show percentage in menu bar                         [ON]│
│ Only show active limit                              [ON]│
│ Launch at login                                    [OFF]│
│ OpenCode Go API key                   Keychain  [Save]  │
├─────────────────────────────────────────────────────────┤
│ CONNECTED ACCOUNTS                                      │
│ [C] Claude Code                                 Max 20× │
│ [O] OpenCode Go                                      Go │
│ [G] Antigravity                               Free tier │
├─────────────────────────────────────────────────────────┤
│ Warn me at                                 ====●=== 80% │
└─────────────────────────────────────────────────────────┘
```

---

## Installation

### Option 1: Homebrew

Install via Homebrew:

```bash
brew tap 4nkitd/tap
brew install headroom
```

*Or via single command:*

```bash
brew install 4nkitd/tap/headroom
```

---

### Option 2: Direct Binary Download (macOS ARM64 / Apple Silicon)

1. Download the latest release binary from the [Releases](https://github.com/4nkitd/headroom/releases/tag/v0.1.0) page:
   ```bash
   curl -LO https://github.com/4nkitd/headroom/releases/download/v0.1.0/headroom-v0.1.0-macos-arm64.zip
   ```
2. Unzip and run:
   ```bash
   unzip headroom-v0.1.0-macos-arm64.zip
   ./headroom
   ```

---

### Option 3: Build from Source

#### Prerequisites
- macOS 10.15+ (Apple Silicon or Intel)
- Rust toolchain (`rustc`, `cargo` 1.85+)

#### Steps

```bash
# Clone repository
git clone https://github.com/4nkitd/headroom.git
cd headroom

# Build release binary
cargo build --release

# Run Headroom
./target/release/headroom
```

---

## Configuration & Usage

1. Launch **Headroom** — the status icon (`G 9%` or `C 32%`) will appear in your macOS menu bar.
2. **Click the menu-bar icon** to expand the popover panel.
3. **Preferences**:
   - Click `Preferences…` to configure your OpenCode Go API key or toggle menu-bar percentages.
   - API keys are stored in the macOS Keychain (`Headroom` service).
4. Press `⌘R` inside the popover or click `Refresh now` to manually update quota status.

---

## License

Apache-2.0 © [4nkitd](https://github.com/4nkitd)
