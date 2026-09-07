# Kitty Emacs copy mode

A keyboard-driven copy mode for [kitty](https://sw.kovidgoyal.net/kitty/), using
Emacs-style keys. Kitty passes its scrollback on standard input; the pager lets
you move through it, search, select a region, and copy that region to the system
clipboard.

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

Copied text is sent to the system clipboard using OSC 52.

### Search

| Key | Action |
| --- | --- |
| `C-s` | Start a forward search; press again for the next match |
| `C-r` | Start a backward search; press again for the previous match |
| `Enter` | Finish entering the search |
| `C-g`, `C-c`, or `Escape` | Cancel search input |

Searches use regular expressions, update while you type, and use smart case by
default.

### Exit

Press `q`, `C-c`, `C-[`, or `Escape` outside search input.

## Options

Run `kitty-copy-pager --help` for display, color, tab width, and search options.

> [!WARNING]
> Resizing while the pager is open is not supported. The pager renders the raw
> scrollback at the terminal size kitty used to produce it.
