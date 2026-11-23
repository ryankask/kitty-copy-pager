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

Set it as kitty's scrollback pager:

```conf
scrollback_pager kitty-copy-pager
map cmd+r show_scrollback
map --when-focus-on cmdline:kitty-copy-pager cmd+r
```

The conditional empty mapping passes `Cmd-r` through while the pager is focused.
Without it, kitty handles the shortcut itself and opens nested pager overlays.
Apply the same conditional empty mapping to any other shortcut that invokes
`show_scrollback` or `show_last_command_output`.

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
