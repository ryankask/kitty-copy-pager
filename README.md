# Kitty Emacs copy mode

A tmux-style copy mode for [kitty](https://sw.kovidgoyal.net/kitty/) with Emacs
keybindings. Kitty passes its scrollback on standard input. Use the pager to
move through it, search, select a region, and copy it to the system clipboard.

This is a fork of
[BasileBux/kitty-copy-pager](https://github.com/BasileBux/kitty-copy-pager).

## Installation

Build and install the pager with Rust:

```bash
cargo install --path .
```

Set the pager and bind the two copy-mode shortcuts in `kitty.conf`.

```conf
scrollback_pager kitty-copy-pager
map cmd+r launch --type=overlay --stdin-source=@screen_scrollback --stdin-add-formatting --stdin-add-line-wrap-markers kitty-copy-pager --wrap-markers
map ctrl+cmd+r launch --type=overlay --stdin-source=@last_cmd_output --stdin-add-formatting --stdin-add-line-wrap-markers kitty-copy-pager --wrap-markers
map --when-focus-on cmdline:kitty-copy-pager cmd+r
map --when-focus-on cmdline:kitty-copy-pager ctrl+cmd+r
```

`Cmd-r` opens the full scrollback. `Ctrl-Cmd-r` opens the last command's output
with kitty shell integration. These mappings pass kitty's wrap markers to the
pager. A long URL stays wrapped on screen and copies as one string. Real line
breaks are preserved, even at the right edge of the terminal. The conditional
mappings pass both shortcuts through while the pager is focused.

The built-in `show_scrollback` action converts wraps to newlines before sending
text to the configured pager. Use the `launch` mappings for wrap-aware copying.

On macOS, enable Option as Meta if it is not already enabled:

```conf
macos_option_as_alt yes
```

## Keys

### Movement

| Key | Action |
| --- | --- |
| `C-b` / `C-f` | Previous/next character |
| `C-p` / `C-n` | Previous/next line |
| `M-b` / `M-f` | Previous word/next word end |
| `C-a` / `C-e` | Beginning/end of line |
| `M-m` | First non-whitespace character |
| `M-<` / `M->` | Beginning/end of scrollback |
| `M-v` / `C-v` | Page up/down |
| `C-l` | Recenter the current line |
| Arrow, Home, End, Page Up, Page Down | Equivalent movement |

### Selection and copying

| Key | Action |
| --- | --- |
| `C-Space` | Begin selection |
| `C-g` | Clear the selection and search highlighting |
| `M-w`, `C-w`, or `Enter` | Copy the selection and exit |
| `C-k` | Copy from the cursor to the end of the line and exit |

The pager sends plain text to the system clipboard using OSC 52. ANSI text styles
appear on screen, and tabs expand to the next tab stop in both the display and the
copied text. Copying an empty region leaves the clipboard intact.

### Search

| Key | Action |
| --- | --- |
| `C-s` | Start a forward search; press again for the next match |
| `C-r` | Start a backward search; press again for the previous match |
| `Enter` | Finish entering the search |
| `C-g`, `C-c`, or `Escape` | Cancel search input |

Searches use regular expressions. By default, the cursor moves to a match as you
type. Lowercase queries ignore case, and an uppercase letter makes the search
case-sensitive. You can keep editing the query after repeating a search.
Cancelling returns the cursor and selection to their positions before the
search. After accepting a query, press `C-g` to clear it and start a different
search with `C-s` or `C-r`.

### Exit

Press `q`, `C-c`, `C-[`, or `Escape` outside search input.

## Options

Run `kitty-copy-pager --help` for display, color, tab width, and search options.

> [!WARNING]
> Resizing while the pager is open is not supported. The pager uses the terminal
> dimensions at startup. When a line exceeds that width, moving the cursor along
> it reveals the rest of the line.
