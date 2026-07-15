# herdr-imebox

A pop-up text box for typing CJK (Japanese, Chinese, Korean) text into AI
agents running inside [herdr](https://herdr.dev) — without IME accidents.

## Why

Typing with an IME inside a terminal agent (Claude Code, codex, etc.) has two
chronic problems:

- The Enter key both confirms IME conversion and submits the message, so a
  mistimed Enter sends a half-written prompt.
- Composed text does not reach the terminal until the conversion is confirmed,
  so if the agent redraws or pops an interactive prompt mid-composition, your
  draft is wiped.

herdr-imebox opens a small dedicated pane where Enter only inserts a newline.
When you are done, one keystroke sends the whole message to the agent pane.

## Usage

1. Focus the agent pane you are talking to and press the bound key
   (e.g. `prefix+i`).
2. A text box opens below the pane. Type freely — Enter is just a newline.
3. `Ctrl+Enter` sends the text (plus Enter) to the agent and closes the box.
   `Ctrl+D` also sends, and is the fallback on terminals without the kitty
   keyboard protocol, where Ctrl+Enter is indistinguishable from Enter (the
   status bar shows which send key is active).
4. `Ctrl+C` / `Esc Esc` closes the box, saving your draft. It is restored the
   next time the box opens.

## Install

Requires a Rust toolchain (stable).

```sh
git clone <this-repository>
cd herdr-imebox
cargo build --release
```

Then add a key binding to `~/.config/herdr/config.toml`:

```toml
[[keys.command]]
key = "prefix+i"
type = "shell"
command = "/path/to/herdr-imebox/target/release/imebox launch"
```

Apply it with `herdr server reload-config`.

Optionally, also enable herdr's built-in IME candidate-window fix for normal
typing:

```toml
[experimental]
reveal_hidden_cursor_for_cjk_ime = true
```

## How it works

A single binary with two subcommands:

- `imebox launch` runs from the key binding, records the currently focused
  pane as the send target, and opens `imebox edit <target>` in a 25% split
  below it. If a box is already open, it focuses that box instead of opening
  another.
- `imebox edit` is a small ratatui TUI. It keeps the hardware terminal cursor
  at the edit position so IME candidate windows track it correctly.
- Multi-line text is wrapped in bracketed-paste sequences, so agents such as
  Claude Code receive it as a single message with embedded newlines.
- If sending fails (e.g. the target pane is gone), the draft is kept and an
  error is shown in the status bar. Drafts and the lock file live in
  `~/.cache/herdr-imebox/`.
