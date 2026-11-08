# Privacy

_Last updated: 2026-11-08_

Kaan's Terminal is a desktop app. Almost everything it does happens on your own computer. This page lists exactly
what stays local, what is sent to our server, and why.

## Stays on your computer (never uploaded)

- **Your AI sessions:** prompts, replies, code, transcripts, file names and paths, tool calls, tokens, costs.
  The app only **reads** the session files of Claude Code (`~/.claude`), Codex (`~/.codex`) and other tools; it never
  writes to them and never reads Claude Code's `.key` files.
- **Your terminals:** everything you type and everything they print.
- **Git:** repositories, diffs, commit messages. Git commands run with your own `git` and your own credentials.
- **Settings, themes, layout, recent folders, the spending ledger and the daily activity log** (`activity.json`).
- **Error log** (`errors.log`, in the app's data folder). Crashes and freezes are written there, with secrets masked.
  It is not sent anywhere; you can attach it to a bug report yourself.
- Anything shown in the interface first passes through a mask that hides known secret formats (API keys, tokens,
  passwords in URLs).

## Sent to our server (only for social features)

The server lives at `kaansterminal.karakuslabs.com`. It is used for the hacker name, friends, messages, the
leaderboard, announcements and feedback. If the server is unreachable, the app keeps working offline.

| Data | Why |
|---|---|
| A **salted SHA-256 hash** of your machine's hardware ID (never the ID itself) | to recognise the same device |
| A random **device token** (the server stores only its SHA-256 hash) | to sign you in |
| Your **hacker name** (`name#tag`), chosen theme, OS name, app version | profile and leaderboard |
| Once a minute while the app runs: **"active or not"** and your **time-zone offset** | XP, daily active time, streaks |
| Your **presence** (online / away / invisible) | shown to friends |
| Friend requests and friendships | friends list |
| **Messages** — end-to-end encrypted (NaCl box, X25519 + XSalsa20-Poly1305). The server stores only ciphertext and public keys; it cannot read them. Private keys stay on your device (`keys.json`). | chat |
| **Feedback** you choose to send, with your name#tag and app version | bug reports |

Privacy switches: **hide my active time** (friends and leaderboard don't see it) and **hide my name** (you appear as
"Anonymous" on the leaderboard). Invisible presence shows you offline to friends.

Technical notes:
- Requests pass through Cloudflare; the server and Cloudflare see your IP address. The IP is used for rate limiting
  and may appear in short-lived server logs. It is not stored with your profile.
- No analytics, tracking or advertising libraries are included. Nothing is sold or shared.

## Optional features that talk to other services (only when you use them)

- **✧ AI commit message** runs **your own** Claude Code (`claude -p`) with your own sign-in. The staged diff (secrets
  masked) is sent to Anthropic by Claude Code, under Anthropic's terms — the same as using Claude Code yourself.
- **Integrations** (Claude status line, theme, token savers) change other tools' settings only after you turn them on,
  with a backup first, and can be removed from the Integrations screen.
- **Updates** are downloaded from our update server and verified with a signature before installing.

## Deleting your data

- Local data: quit the app and delete its data folder
  (macOS `~/Library/Application Support/com.kaankarakus.kaans-terminal`,
  Windows `%APPDATA%\com.kaankarakus.kaans-terminal`).
- Server data: send a deletion request through the in-app feedback button with your `name#tag`; the device, profile,
  activity, friendships and messages are removed together.

## Contact

Use the feedback button in the app.
