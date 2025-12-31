//! # print-break
//!
//! A simple debugging macro that pretty-prints variables and pauses execution.
//! No debugger needed - just prints to stderr and waits for Enter.
//!
//! ## Features
//!
//! - Pretty-prints any `Debug` type
//! - Auto-detects and formats JSON, TOML, YAML strings
//! - Shows file:line location
//! - Pauses execution until you press Enter
//! - **Compiles to nothing in release builds**
//! - **Disable at runtime with `PRINT_BREAK=0`**
//! - **Interactive: Enter=continue, q=quit, s=skip remaining**
//!
//! ## Usage
//!
//! ```rust,no_run
//! use print_break::print_break;
//!
//! let x = 42;
//! let name = "ferris";
//! let json = r#"{"user": "alice", "id": 123}"#;
//!
//! print_break!(x, name, json);
//! ```
//!
//! ## Environment Variables
//!
//! - `PRINT_BREAK=0` - Disable all breakpoints
//! - `PRINT_BREAK=1` - Enable breakpoints (default)
//! - `PRINT_BREAK_DEPTH=N` - Max nesting depth before collapsing (default: 4)
//!
//! ## Interactive Controls
//!
//! When paused at a breakpoint:
//! - **Enter** - Continue to next breakpoint
//! - **q** - Quit the program immediately
//! - **s** - Skip all remaining breakpoints

use std::fmt::Debug;
use std::io::IsTerminal;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Global flag to skip all remaining breakpoints
static SKIP_ALL: AtomicBool = AtomicBool::new(false);

/// Global breakpoint counter
static BREAK_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Maximum lines to show before truncating
const MAX_LINES: usize = 50;

/// Check if print-break is enabled via environment variable
#[doc(hidden)]
pub fn is_enabled() -> bool {
    if SKIP_ALL.load(Ordering::Relaxed) {
        return false;
    }
    match std::env::var("PRINT_BREAK") {
        Ok(val) => !matches!(val.as_str(), "0" | "false" | "no" | "off"),
        Err(_) => true, // Enabled by default
    }
}

/// Check if we're running in a TTY (interactive terminal)
#[doc(hidden)]
pub fn is_tty() -> bool {
    std::io::stderr().is_terminal() && std::io::stdin().is_terminal()
}

/// Get and increment breakpoint counter
#[doc(hidden)]
pub fn next_break_id() -> usize {
    BREAK_COUNT.fetch_add(1, Ordering::Relaxed) + 1
}

/// Color code wrapper - returns empty string if not TTY
#[doc(hidden)]
pub fn color(code: &str) -> &str {
    if is_tty() { code } else { "" }
}

/// Reset color code - returns empty string if not TTY
#[doc(hidden)]
pub fn reset() -> &'static str {
    if is_tty() { "\x1b[0m" } else { "" }
}

/// Set the skip-all flag
#[doc(hidden)]
pub fn set_skip_all(skip: bool) {
    SKIP_ALL.store(skip, Ordering::Relaxed);
}

/// Attempts to format a value as pretty JSON/TOML/YAML if it's a config string.
/// Falls back to Debug formatting otherwise.
/// Truncates output if it exceeds MAX_LINES.
#[doc(hidden)]
pub fn format_value<T: Debug>(value: &T) -> String {
    let debug_str = format!("{:?}", value);
    let raw_output;

    // Check if it's a string
    if let Some(inner) = debug_str.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        // Unescape the string
        let unescaped = inner
            .replace("\\\"", "\"")
            .replace("\\n", "\n")
            .replace("\\t", "\t")
            .replace("\\\\", "\\");

        let trimmed = unescaped.trim();

        // Try JSON first (most specific - must start with { or [)
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&unescaped) {
                if let Ok(pretty) = serde_json::to_string_pretty(&json) {
                    raw_output = format!("\x1b[90m(json)\x1b[0m\n{}", pretty);
                    return truncate_output(&raw_output);
                }
            }
        }

        // Try TOML (look for key = value or [section] patterns)
        if trimmed.contains(" = ") || trimmed.contains("]\n") || trimmed.starts_with('[') {
            if let Ok(toml_val) = toml::from_str::<toml::Value>(&unescaped) {
                if let Ok(pretty) = toml::to_string_pretty(&toml_val) {
                    raw_output = format!("\x1b[90m(toml)\x1b[0m\n{}", pretty);
                    return truncate_output(&raw_output);
                }
            }
        }

        // Try YAML (look for key: value patterns, but not just any colon)
        if trimmed.contains(": ") || trimmed.contains(":\n") {
            if let Ok(yaml_val) = serde_yaml::from_str::<serde_yaml::Value>(&unescaped) {
                // Only use YAML if it parsed into something structured (not just a string)
                if yaml_val.is_mapping() || yaml_val.is_sequence() {
                    if let Ok(pretty) = serde_yaml::to_string(&yaml_val) {
                        raw_output = format!("\x1b[90m(yaml)\x1b[0m\n{}", pretty.trim());
                        return truncate_output(&raw_output);
                    }
                }
            }
        }

        // For plain text strings, show with newlines and word wrap
        raw_output = format!("\x1b[90m(string, {} chars)\x1b[0m\n{}", unescaped.len(), word_wrap(&unescaped, 80));
        return truncate_output(&raw_output);
    }

    // Fall back to pretty Debug format with colorization
    let debug_output = format!("{:#?}", value);
    raw_output = colorize_debug(&debug_output);
    truncate_output(&raw_output)
}

/// Format value without truncation (for "more" output)
#[doc(hidden)]
pub fn format_value_full<T: Debug>(value: &T) -> String {
    let debug_str = format!("{:?}", value);

    // Check if it's a string
    if let Some(inner) = debug_str.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        // Unescape the string
        let unescaped = inner
            .replace("\\\"", "\"")
            .replace("\\n", "\n")
            .replace("\\t", "\t")
            .replace("\\\\", "\\");

        let trimmed = unescaped.trim();

        // Try JSON
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&unescaped) {
                if let Ok(pretty) = serde_json::to_string_pretty(&json) {
                    return pretty;
                }
            }
        }

        // Try TOML
        if trimmed.contains(" = ") || trimmed.contains("]\n") || trimmed.starts_with('[') {
            if let Ok(toml_val) = toml::from_str::<toml::Value>(&unescaped) {
                if let Ok(pretty) = toml::to_string_pretty(&toml_val) {
                    return pretty;
                }
            }
        }

        // Try YAML
        if trimmed.contains(": ") || trimmed.contains(":\n") {
            if let Ok(yaml_val) = serde_yaml::from_str::<serde_yaml::Value>(&unescaped) {
                if yaml_val.is_mapping() || yaml_val.is_sequence() {
                    if let Ok(pretty) = serde_yaml::to_string(&yaml_val) {
                        return pretty.trim().to_string();
                    }
                }
            }
        }

        // Plain text with word wrap
        return word_wrap(&unescaped, 100);
    }

    // Colorize debug output
    colorize_debug(&format!("{:#?}", value))
}

/// Default maximum nesting depth before collapsing
const DEFAULT_MAX_DEPTH: usize = 4;

/// Get max depth from environment variable or use default
fn max_depth() -> usize {
    std::env::var("PRINT_BREAK_DEPTH")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MAX_DEPTH)
}

/// Colorize Debug output for structs/enums
fn colorize_debug(s: &str) -> String {
    let tty = is_tty();
    if !tty {
        return s.to_string();
    }

    let (green, cyan, yellow, magenta, white, gray, reset) = (
        "\x1b[1;32m", // struct/enum names
        "\x1b[36m",   // field names
        "\x1b[33m",   // numbers
        "\x1b[35m",   // strings
        "\x1b[37m",   // other values
        "\x1b[90m",   // punctuation/guides
        "\x1b[0m",
    );

    let mut result = String::new();
    let lines: Vec<&str> = s.lines().collect();
    let mut current_depth: usize = 0;
    let mut skip_until_depth: Option<usize> = None;

    for line in lines {
        let trimmed = line.trim_start();
        let indent_count = line.len() - trimmed.len();
        let indent_level = indent_count / 4;

        // Track depth changes
        let opens = trimmed.ends_with('{') || trimmed.ends_with('[') || trimmed.ends_with("({");
        let closes = trimmed.starts_with('}') || trimmed.starts_with(']') || trimmed.starts_with(')');

        if closes {
            current_depth = current_depth.saturating_sub(1);
        }

        // Check if we're skipping due to depth
        if let Some(skip_depth) = skip_until_depth {
            if current_depth < skip_depth {
                skip_until_depth = None;
            } else {
                if opens {
                    current_depth += 1;
                }
                continue;
            }
        }

        // If we're at max depth and opening a new block, collapse it
        if opens && current_depth >= max_depth() {
            // Add indentation guides
            for _ in 0..indent_level {
                result.push_str(&format!("{}│{} ", gray, reset));
            }

            // Show collapsed version
            let name = trimmed.trim_end_matches(|c| c == '{' || c == '[' || c == '(' || c == ' ');
            if !name.is_empty() {
                result.push_str(&format!("{}{}{} {}{{ ... }}{}", green, name, reset, gray, reset));
            } else {
                result.push_str(&format!("{}[ ... ]{}", gray, reset));
            }
            result.push('\n');

            skip_until_depth = Some(current_depth);
            current_depth += 1;
            continue;
        }

        // Add indentation guides
        for _ in 0..indent_level {
            result.push_str(&format!("{}│{} ", gray, reset));
        }

        // Colorize the content
        if opens {
            // Struct/enum name line: "User {" or "Some(" or "["
            let name = trimmed.trim_end_matches(|c| c == '{' || c == '[' || c == '(' || c == ' ');
            let bracket = trimmed.chars().last().unwrap_or(' ');
            if !name.is_empty() {
                result.push_str(&format!("{}{}{} {}{}{}", green, name, reset, gray, bracket, reset));
            } else {
                result.push_str(&format!("{}{}{}", gray, bracket, reset));
            }
            current_depth += 1;
        } else if closes || trimmed.ends_with("},") || trimmed.ends_with("],") || trimmed.ends_with("),") {
            // Closing brace
            result.push_str(&format!("{}{}{}", gray, trimmed, reset));
        } else if trimmed.contains(": ") {
            // Field: value line
            if let Some(colon_pos) = trimmed.find(": ") {
                let field = &trimmed[..colon_pos];
                let value = &trimmed[colon_pos + 2..];
                let colored_value = colorize_value(value, yellow, magenta, white, gray, reset);
                result.push_str(&format!("{}{}{}{}: {}", cyan, field, reset, gray, colored_value));
            } else {
                result.push_str(trimmed);
            }
        } else {
            // Array element or other
            let colored = colorize_value(trimmed, yellow, magenta, white, gray, reset);
            result.push_str(&colored);
        }
        result.push('\n');
    }

    result.trim_end().to_string()
}

/// Colorize a single value
fn colorize_value(s: &str, yellow: &str, magenta: &str, white: &str, gray: &str, reset: &str) -> String {
    let trimmed = s.trim_end_matches(',');
    let has_comma = s.ends_with(',');
    let comma = if has_comma { format!("{},{}", gray, reset) } else { String::new() };

    if trimmed.starts_with('"') {
        // String value
        format!("{}{}{}{}", magenta, trimmed, reset, comma)
    } else if trimmed.parse::<f64>().is_ok() || trimmed.starts_with('-') && trimmed[1..].parse::<f64>().is_ok() {
        // Number
        format!("{}{}{}{}", yellow, trimmed, reset, comma)
    } else if trimmed == "true" || trimmed == "false" {
        // Boolean
        format!("{}{}{}{}", yellow, trimmed, reset, comma)
    } else if trimmed == "None" || trimmed.starts_with("Some(") {
        // Option
        format!("{}{}{}{}", white, trimmed, reset, comma)
    } else {
        format!("{}{}{}{}", white, trimmed, reset, comma)
    }
}

/// Word wrap text at specified width
fn word_wrap(s: &str, width: usize) -> String {
    let mut result = String::new();
    for line in s.lines() {
        if line.len() <= width {
            result.push_str(line);
            result.push('\n');
        } else {
            let mut current_line = String::new();
            for word in line.split_whitespace() {
                if current_line.is_empty() {
                    current_line = word.to_string();
                } else if current_line.len() + 1 + word.len() <= width {
                    current_line.push(' ');
                    current_line.push_str(word);
                } else {
                    result.push_str(&current_line);
                    result.push('\n');
                    current_line = word.to_string();
                }
            }
            if !current_line.is_empty() {
                result.push_str(&current_line);
                result.push('\n');
            }
        }
    }
    result.trim_end().to_string()
}

/// Truncate output if it exceeds MAX_LINES
fn truncate_output(s: &str) -> String {
    let lines: Vec<&str> = s.lines().collect();
    if lines.len() > MAX_LINES {
        let mut result: Vec<&str> = lines[..MAX_LINES].to_vec();
        result.push(&format!("\x1b[90m... ({} more lines)\x1b[0m", lines.len() - MAX_LINES));
        // Can't use format! in const context, so we do this differently
        let truncated: String = lines[..MAX_LINES].join("\n");
        format!("{}\n\x1b[90m... ({} more lines)\x1b[0m", truncated, lines.len() - MAX_LINES)
    } else {
        s.to_string()
    }
}

/// Stored full output for "show more" functionality
static LAST_FULL_OUTPUT: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Store full output for potential "show more"
#[doc(hidden)]
pub fn store_full_output(output: String) {
    if let Ok(mut guard) = LAST_FULL_OUTPUT.lock() {
        *guard = Some(output);
    }
}

/// Handle user input at breakpoint. Returns true if should continue, false if should quit.
#[doc(hidden)]
pub fn handle_input() -> bool {
    use std::io::{self, BufRead, Write};

    // If not a TTY, don't pause - just continue (for CI/piped output)
    if !is_tty() {
        eprintln!("(non-interactive mode, continuing...)");
        return true;
    }

    loop {
        eprint!("\x1b[90m[Enter=continue, m=more, s=skip all, q=quit]\x1b[0m ");
        io::stderr().flush().unwrap();

        let stdin = io::stdin();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).is_ok() {
            let input = line.trim().to_lowercase();
            match input.as_str() {
                "q" | "quit" => {
                    eprintln!("\x1b[1;31mQuitting...\x1b[0m");
                    std::process::exit(0);
                }
                "s" | "skip" => {
                    eprintln!("\x1b[1;33mSkipping remaining breakpoints...\x1b[0m");
                    set_skip_all(true);
                    break;
                }
                "m" | "more" => {
                    // Show full output
                    if let Ok(guard) = LAST_FULL_OUTPUT.lock() {
                        if let Some(ref full) = *guard {
                            eprintln!("\n\x1b[1;33m─── Full Output ───\x1b[0m");
                            for line in full.lines() {
                                eprintln!("\x1b[37m{}\x1b[0m", line);
                            }
                            eprintln!("\x1b[1;33m───────────────────\x1b[0m\n");
                        } else {
                            eprintln!("\x1b[90m(no truncated output to show)\x1b[0m");
                        }
                    }
                    continue; // Ask for input again
                }
                _ => break // Continue
            }
        } else {
            break;
        }
    }
    eprintln!();
    true
}

/// Pretty-prints variables and pauses execution until Enter is pressed.
///
/// # Features
///
/// - Compiles to nothing in release builds
/// - Can be disabled with `PRINT_BREAK=0` environment variable
/// - Interactive: Enter=continue, q=quit, s=skip all remaining
///
/// # Examples
///
/// ```rust,no_run
/// use print_break::print_break;
///
/// let user_id = 123;
/// let items = vec!["apple", "banana"];
/// let json = r#"{"status": "ok"}"#;
///
/// // Print single variable
/// print_break!(user_id);
///
/// // Print multiple variables
/// print_break!(user_id, items, json);
///
/// // Print with no variables (just pause)
/// print_break!();
/// ```
#[macro_export]
#[cfg(debug_assertions)]
macro_rules! print_break {
    () => {{
        if $crate::is_enabled() {
            use std::io::Write;

            let break_id = $crate::next_break_id();
            let location = format!("{}:{}", file!(), line!());
            let width = 50;
            let tty = $crate::is_tty();

            let (yellow, cyan, reset) = if tty {
                ("\x1b[1;33m", "\x1b[36m", "\x1b[0m")
            } else {
                ("", "", "")
            };

            eprintln!();
            eprintln!("{}┌─ BREAK #{} {}{}", yellow, break_id, "─".repeat(width - 12 - break_id.to_string().len()), reset);
            eprintln!("{}│{} {}{:<width$}{}{}│{}", yellow, reset, cyan, location, reset, yellow, reset, width = width - 2);
            eprintln!("{}└{}{}", yellow, "─".repeat(width), reset);

            $crate::handle_input();
        }
    }};
    ($($var:expr),+ $(,)?) => {{
        if $crate::is_enabled() {
            use std::io::Write;

            let break_id = $crate::next_break_id();
            let location = format!("{}:{}", file!(), line!());
            let width = 50;
            let tty = $crate::is_tty();

            let (yellow, cyan, green, white, reset) = if tty {
                ("\x1b[1;33m", "\x1b[36m", "\x1b[1;32m", "\x1b[37m", "\x1b[0m")
            } else {
                ("", "", "", "", "")
            };

            // Collect full output for "more" option
            let mut full_output = String::new();

            eprintln!();
            eprintln!("{}┌─ BREAK #{} {}{}", yellow, break_id, "─".repeat(width - 12 - break_id.to_string().len()), reset);
            eprintln!("{}│{} {}{:<width$}{}{}│{}", yellow, reset, cyan, location, reset, yellow, reset, width = width - 2);
            eprintln!("{}├{}{}", yellow, "─".repeat(width), reset);

            $(
                let formatted = $crate::format_value(&$var);
                let name = stringify!($var);

                // Store untruncated version
                full_output.push_str(&format!("{} = {}\n\n", name, $crate::format_value_full(&$var)));

                if formatted.contains('\n') {
                    eprintln!("{}│{} {}{}{}=", yellow, reset, green, name, reset);
                    for line in formatted.lines() {
                        eprintln!("{}│{}   {}{}{}", yellow, reset, white, line, reset);
                    }
                } else {
                    eprintln!("{}│{} {}{}{} = {}{}{}", yellow, reset, green, name, reset, white, formatted, reset);
                }
            )+

            $crate::store_full_output(full_output);

            eprintln!("{}└{}{}", yellow, "─".repeat(width), reset);
            $crate::handle_input();
        }
    }};
}

/// Conditional breakpoint - only triggers if condition is true.
///
/// # Examples
///
/// ```rust,no_run
/// use print_break::print_break_if;
///
/// for i in 0..100 {
///     print_break_if!(i == 50, i);  // Only breaks when i is 50
/// }
///
/// let x = 42;
/// print_break_if!(x > 10, x);  // Breaks because x > 10
/// ```
#[macro_export]
#[cfg(debug_assertions)]
macro_rules! print_break_if {
    ($cond:expr) => {{
        if $cond {
            $crate::print_break!();
        }
    }};
    ($cond:expr, $($var:expr),+ $(,)?) => {{
        if $cond {
            $crate::print_break!($($var),+);
        }
    }};
}

/// In release builds, print_break_if! compiles to nothing
#[macro_export]
#[cfg(not(debug_assertions))]
macro_rules! print_break_if {
    ($cond:expr) => {{}};
    ($cond:expr, $($var:expr),+ $(,)?) => {{}};
}

/// In release builds, print_break! compiles to nothing
#[macro_export]
#[cfg(not(debug_assertions))]
macro_rules! print_break {
    () => {{}};
    ($($var:expr),+ $(,)?) => {{}};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_json_string() {
        let json = r#"{"name": "test", "value": 42}"#;
        let formatted = format_value(&json);
        assert!(formatted.contains("\"name\": \"test\""));
        assert!(formatted.contains('\n')); // Should be pretty-printed
    }

    #[test]
    fn format_non_json() {
        let x = 42;
        let formatted = format_value(&x);
        assert_eq!(formatted, "42");
    }

    #[test]
    fn format_struct() {
        #[derive(Debug)]
        struct Test { a: i32, b: String }

        let t = Test { a: 1, b: "hello".to_string() };
        let formatted = format_value(&t);
        assert!(formatted.contains("Test"));
    }

    #[test]
    fn truncation_works() {
        let long_vec: Vec<i32> = (0..1000).collect();
        let formatted = format_value(&long_vec);
        assert!(formatted.contains("more lines"));
    }

    #[test]
    fn env_var_disable() {
        std::env::set_var("PRINT_BREAK", "0");
        assert!(!is_enabled());
        std::env::set_var("PRINT_BREAK", "1");
        assert!(is_enabled());
        std::env::remove_var("PRINT_BREAK");
    }
}
