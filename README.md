# print-break

A simple Rust debugging macro that pretty-prints variables and pauses execution. No debugger needed.

## Features

- **Pretty-prints any `Debug` type** with syntax highlighting
- **Auto-detects JSON, TOML, YAML** strings and formats them with colors
- **Pauses execution** until you press Enter
- **Compiles to nothing in release builds** - zero overhead in production
- **Disable at runtime** with `PRINT_BREAK=0`
- **Interactive controls**: continue, quit, skip, more, help
- **Stack traces** - press `t` to see how you got there
- **Clipboard support** - press `c` to copy values
- **Elapsed time** - see time between breakpoints
- **Conditional breakpoints** with `print_break_if!`
- **Non-TTY safe** - won't hang in CI/piped output
- **Customizable borders** - rounded, sharp, double, or ASCII

## Installation

```toml
[dependencies]
print-break = "0.1"
```

## Usage

```rust
use print_break::{print_break, print_break_if};

fn main() {
    let user_id = 42;
    let name = "ferris";
    let items = vec![1, 2, 3];

    // Basic breakpoint
    print_break!(user_id, name, items);

    // Conditional breakpoint (great for loops)
    for i in 0..100 {
        print_break_if!(i == 50, i);
    }

    // JSON strings are auto-formatted
    let json = r#"{"status": "ok", "data": [1, 2, 3]}"#;
    print_break!(json);
}
```

## Output

```
┌─ BREAK #1 ─────────────────────────────────────
│ src/main.rs:8                                  │
├────────────────────────────────────────────────
│ user_id = 42
│ name = "ferris"
│ items = [1, 2, 3]
└────────────────────────────────────────────────
[Enter=continue, m=more, s=skip all, q=quit]
```

## Interactive Controls

When paused at a breakpoint:

| Key | Action |
|-----|--------|
| **Enter** | Continue to next breakpoint |
| **m** | Show full output (if truncated) |
| **t** | Show stack trace |
| **c** | Copy value to clipboard |
| **s** | Skip all remaining breakpoints |
| **q** | Quit the program |
| **h / ?** | Show help |

## Environment Variables

```bash
# Disable all breakpoints
PRINT_BREAK=0 cargo run

# Re-enable (default)
PRINT_BREAK=1 cargo run

# Set max nesting depth for structs (default: 4)
PRINT_BREAK_DEPTH=6 cargo run

# Show all nesting (no collapse)
PRINT_BREAK_DEPTH=999 cargo run

# Border styles: rounded (default), sharp, double, ascii
PRINT_BREAK_STYLE=double cargo run
PRINT_BREAK_STYLE=ascii cargo run
```

## CI / Non-Interactive Mode

When stderr is not a TTY (piped to file, running in CI), print-break automatically:
- Disables colors
- Skips the pause (won't hang your CI)
- Still prints the debug output for logging

## Conditional Breakpoints

Perfect for debugging loops:

```rust
use print_break::print_break_if;

for i in 0..1000 {
    // Only break when i is 500
    print_break_if!(i == 500, i);

    // Break when condition is met
    print_break_if!(some_value > threshold, some_value, threshold);
}
```

## Release Builds

In release builds (`cargo build --release`), all `print_break!` and `print_break_if!` calls compile to nothing - zero runtime overhead.

## Format Detection

Strings are automatically detected and pretty-printed:

- **JSON** - Objects and arrays
- **TOML** - Configuration files
- **YAML** - Configuration files
- **Plain text** - Word-wrapped at 80 characters

Long output is truncated at 50 lines. Press `m` to see the full output.

## Editor Integration

### Neovim

Copy `editors/nvim/print-break.lua` to your Neovim config:

```bash
cp editors/nvim/print-break.lua ~/.config/nvim/after/plugin/
```

**Keybindings (Rust files only):**

| Key | Mode | Action |
|-----|------|--------|
| `<leader>pb` | Normal | Insert `print_break!()` with word under cursor |
| `<leader>pb` | Visual | Wrap selection in `print_break!()` |
| `<leader>pB` | Normal | Insert `print_break_if!(condition, vars)` |
| `<leader>pc` | Normal | Remove all `print_break!` from current buffer |
| `<leader>pC` | Normal | Remove all `print_break!` from all Rust files |

**Smart features:**
- Automatically captures the variable under cursor
- Appends to existing `print_break!` if one exists on the next line
- Finds end of multi-line statements before inserting

## License

MIT
