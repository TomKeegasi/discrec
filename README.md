<div align="center">

# DiscRec

**Beautiful, minimal Discord audio recorder.**

[![CI](https://github.com/jmpijll/discrec/actions/workflows/ci.yml/badge.svg)](https://github.com/jmpijll/discrec/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

</div>

---

DiscRec captures Discord audio with a single click. No configuration, no bloat — just press record.

Built with [Tauri v2](https://tauri.app) (Rust) and React. Cross-platform: Windows, macOS, and Linux.

## Features

- **One-click recording** — press record to capture Discord audio instantly
- **Multiple formats** — WAV (lossless), FLAC (lossless compressed), MP3 (192 kbps)
- **Discord bot integration** — connect a bot to record per-speaker audio tracks with Discord display names
- **Auto-record** — automatically start recording when someone joins a voice channel (bot mode)
- **Per-process capture** — records only Discord audio, not your entire system (Windows)
- **Silence trim** — automatically skips leading silence in recordings when enabled
- **Auto-updater** — get notified and install updates directly from the app
- **Configurable output directory** — choose where recordings are saved
- **Keyboard shortcuts** — Ctrl+R to record, Ctrl+S or Escape to stop (changed in settings)
- **Dark / light theme** — switch in settings, persisted across sessions
- **Live audio meter** — real-time level visualization with smooth decay and peak hold
- **Recording history** — browse, open folder, or delete past recordings from settings
- **System tray** — record, stop, and quit from the tray icon (with visible icon on Windows)
- **Minimalist modern design** — clean layout with spacious components and visual hierarchy

## Install

Download the latest release for your platform from the [Releases](https://github.com/jmpijll/discrec/releases) page. DiscRec will notify you when updates are available.

| Platform | Format |
|----------|--------|
| Windows | `.msi` / `.exe` |
| macOS | `.dmg` |
| Linux | `.AppImage` / `.deb` |

> **macOS note:** System audio capture requires [BlackHole](https://existential.audio/blackhole/) or a similar virtual audio device.

## Discord bot setup (per-speaker recording)

By default DiscRec records whatever Discord plays on your computer, mixed into a single track. If you connect your own Discord bot instead, the bot joins a voice channel and saves **a separate track for each speaker**, named after their Discord display name.

The bot runs inside DiscRec on your computer, so there is nothing to host. You only need a bot account, which takes about five minutes to set up.

### 1. Create the bot

1. Open the [Discord Developer Portal](https://discord.com/developers/applications) and sign in with your Discord account.
2. Click **New Application**, give it a name (for example `DiscRec`), accept the terms and click **Create**.
3. In the left sidebar, open **Bot**.
4. *(Recommended)* Turn **Public Bot** off, so that only you can add the bot to servers.

DiscRec does **not** need any of the *Privileged Gateway Intents* (Presence, Server Members, Message Content). You can leave them all off.

### 2. Copy the bot token

1. Still on the **Bot** page, click **Reset Token** and confirm. If your account uses two-factor authentication, enter your 2FA code.
2. Click **Copy**. Discord shows the token only once; if you lose it, reset it again and use the new one.

> [!WARNING]
> The bot token works like a password for your bot. If it leaks, click **Reset Token** right away.

Make sure you copy the token from the **Bot** page. The *Client Secret* on the OAuth2 page is a different value and won't work.

### 3. Invite the bot to your server

1. In the left sidebar, open **OAuth2** → **URL Generator**.
2. Under **Scopes**, tick **`bot`**.
3. Under **Bot Permissions**, tick:

   | Permission | Needed for |
   |---|---|
   | **View Channels** | Seeing the server's voice channels |
   | **Connect** | Joining the voice channel to record |
   | **Send Messages** | *Optional:* only needed for the **Notify channel** setting, which posts "🔴 Recording started by DiscRec" in the voice channel's chat |

   Don't give the bot *Administrator* rights; it doesn't need them.
4. Copy the **Generated URL** at the bottom of the page, open it in your browser, choose your server and click **Authorize**. You need the *Manage Server* permission on that server to add a bot.

If the bot can't see or join a voice channel later, check that channel's own permission overrides:
a private channel has to allow the bot (or one of its roles) **View Channel** and **Connect** as well.

### 4. Connect the bot in DiscRec

1. Open DiscRec and click the **gear icon** in the bottom-right corner to open **Settings**.
2. In the **Discord** section, paste the token into the **Paste bot token...** field and click **Connect Bot**.
3. When the status shows **Connected**, pick a **Server** and a **Voice Channel**.
4. Close Settings and press the record button. The bot joins the channel and starts recording. Press the button again to stop; the bot leaves and the tracks are saved.

Once the bot is connected, two more options appear in the Discord section:

- **Notify channel**: posts a message in the voice channel's chat when a recording starts, so everyone knows they are being recorded. This needs the *Send Messages* permission.
- **Auto-record**: starts recording when someone joins the selected channel and stops when everyone has left. Other bots are not counted.

DiscRec saves the token in your operating system's secure credential store (Windows Credential Manager, macOS Keychain, or the Secret Service on Linux, e.g. GNOME Keyring or KWallet), not in a plain file. It connects automatically the next time you start the app. To stop using the bot, click **Disconnect** in Settings.

> [!NOTE]
> On Linux, if no Secret Service provider is running, DiscRec can't save the token.
> The bot still works, but you'll need to paste the token again each time you start the app.

## Build from source

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) (LTS)
- [pnpm](https://pnpm.io/)
- Platform dependencies — see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

```bash
git clone https://github.com/jmpijll/discrec.git
cd discrec
pnpm install
pnpm tauri:dev    # development
pnpm tauri:build  # production build
```

## How it works

DiscRec uses OS-level audio capture to record what Discord outputs, so everyone *except you*.

- **Windows:** captures only Discord. Discord must be running.
- **Linux:** sends Discord's audio to a separate capture channel using `pactl` (PipeWire users need `pipewire-pulse`). If DiscRec can't find Discord, it records all system audio instead.
- **macOS:** install [BlackHole](https://existential.audio/blackhole/), create a **Multi-Output Device** (BlackHole + your speakers) in Audio MIDI Setup, and choose it as Discord's output device. DiscRec records everything sent to BlackHole.

To record yourself too, use the Discord bot, which saves a separate track for every speaker.

DiscRec saves every recording in the same folder (configurable in settings).
By default, that folder is a `DiscRec` folder inside your Music folder:

| Platform | Default folder |
|---|---|
| Windows | `C:\Users\<you>\Music\DiscRec` |
| macOS | `~/Music/DiscRec` (`/Users/<you>/Music/DiscRec`) |
| Linux | `~/Music/DiscRec`, or more precisely your XDG Music folder (`$XDG_MUSIC_DIR/DiscRec`). If no Music folder is configured, DiscRec uses `~/DiscRec` instead. |

DiscRec creates the folder automatically the first time you record.

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Roadmap

See [ROADMAP.md](ROADMAP.md) for what's planned.

## License

[MIT](LICENSE)
