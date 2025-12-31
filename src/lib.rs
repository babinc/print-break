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
use std::sync::Mutex;
use std::time::Instant;

/// Global flag to skip all remaining breakpoints
static SKIP_ALL: AtomicBool = AtomicBool::new(false);

/// Global breakpoint counter
static BREAK_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Last breakpoint timestamp for elapsed time
static LAST_BREAK_TIME: Mutex<Option<Instant>> = Mutex::new(None);

/// Border style characters
#[derive(Clone, Copy)]
pub struct BorderStyle {
    pub top_left: char,
    pub top_right: char,
    pub bottom_left: char,
    pub bottom_right: char,
    pub horizontal: char,
    pub vertical: char,
    pub tee_right: char,
}

impl BorderStyle {
    pub const ROUNDED: Self = Self {
        top_left: '╭',
        top_right: '╮',
        bottom_left: '╰',
        bottom_right: '╯',
        horizontal: '─',
        vertical: '│',
        tee_right: '├',
    };

    pub const SHARP: Self = Self {
        top_left: '┌',
        top_right: '┐',
        bottom_left: '└',
        bottom_right: '┘',
        horizontal: '─',
        vertical: '│',
        tee_right: '├',
    };

    pub const DOUBLE: Self = Self {
        top_left: '╔',
        top_right: '╗',
        bottom_left: '╚',
        bottom_right: '╝',
        horizontal: '═',
        vertical: '║',
        tee_right: '╠',
    };

    pub const ASCII: Self = Self {
        top_left: '+',
        top_right: '+',
        bottom_left: '+',
        bottom_right: '+',
        horizontal: '-',
        vertical: '|',
        tee_right: '+',
    };
}

/// Get border style from environment variable
#[doc(hidden)]
pub fn get_border_style() -> BorderStyle {
    match std::env::var("PRINT_BREAK_STYLE").as_deref() {
        Ok("round") | Ok("rounded") => BorderStyle::ROUNDED,
        Ok("sharp") => BorderStyle::SHARP,
        Ok("double") => BorderStyle::DOUBLE,
        Ok("ascii") => BorderStyle::ASCII,
        _ => BorderStyle::ROUNDED,
    }
}

// ============================================================================
// Colors - Centralized ANSI color codes
// ============================================================================

/// ANSI color codes for terminal output
#[derive(Clone, Copy)]
pub struct Colors {
    pub green: &'static str,
    pub cyan: &'static str,
    pub yellow: &'static str,
    pub magenta: &'static str,
    pub white: &'static str,
    pub gray: &'static str,
    pub red: &'static str,
    pub reset: &'static str,
}

impl Colors {
    const TTY: Self = Self {
        green: "\x1b[1;32m",
        cyan: "\x1b[36m",
        yellow: "\x1b[1;33m",
        magenta: "\x1b[35m",
        white: "\x1b[37m",
        gray: "\x1b[90m",
        red: "\x1b[1;31m",
        reset: "\x1b[0m",
    };

    const PLAIN: Self = Self {
        green: "",
        cyan: "",
        yellow: "",
        magenta: "",
        white: "",
        gray: "",
        red: "",
        reset: "",
    };

    /// Get colors based on TTY detection
    #[inline]
    pub fn get() -> Self {
        if is_tty() { Self::TTY } else { Self::PLAIN }
    }
}

/// Format elapsed duration for display
#[doc(hidden)]
pub fn format_elapsed(d: std::time::Duration) -> String {
    let c = Colors::get();
    let micros = d.as_micros();
    if micros < 1000 {
        format!(" {}+{}µs{}", c.gray, micros, c.reset)
    } else if micros < 1_000_000 {
        format!(" {}+{:.1}ms{}", c.gray, micros as f64 / 1000.0, c.reset)
    } else {
        format!(" {}+{:.2}s{}", c.gray, micros as f64 / 1_000_000.0, c.reset)
    }
}

/// Get elapsed time since last breakpoint
#[doc(hidden)]
pub fn get_elapsed() -> Option<std::time::Duration> {
    if let Ok(guard) = LAST_BREAK_TIME.lock() {
        guard.map(|t| t.elapsed())
    } else {
        None
    }
}

/// Update last breakpoint time
#[doc(hidden)]
pub fn update_break_time() {
    if let Ok(mut guard) = LAST_BREAK_TIME.lock() {
        *guard = Some(Instant::now());
    }
}

/// Maximum lines to show before truncating
const MAX_LINES: usize = 50;

/// Colorize JSON output
fn colorize_json(s: &str) -> String {
    let c = Colors::get();
    if c.cyan.is_empty() {
        return s.to_string();
    }

    let (cyan, magenta, yellow, gray, reset) = (c.cyan, c.magenta, c.yellow, c.gray, c.reset);

    let mut result = String::new();
    let mut in_string = false;
    let mut is_key = true;
    let mut chars = s.chars().peekable();
    // Track nesting: true = object (has keys), false = array (no keys)
    let mut context_stack: Vec<bool> = Vec::new();

    while let Some(c) = chars.next() {
        match c {
            '"' if !in_string => {
                in_string = true;
                let color = if is_key { cyan } else { magenta };
                result.push_str(color);
                result.push('"');
            }
            '"' if in_string => {
                result.push('"');
                result.push_str(reset);
                in_string = false;
            }
            ':' if !in_string => {
                result.push_str(gray);
                result.push(':');
                result.push_str(reset);
                is_key = false;
            }
            ',' if !in_string => {
                result.push_str(gray);
                result.push(',');
                result.push_str(reset);
                // After comma, next string is a key only if we're in an object
                is_key = context_stack.last().copied().unwrap_or(true);
            }
            '{' | '[' if !in_string => {
                result.push_str(gray);
                result.push(c);
                result.push_str(reset);
                let is_object = c == '{';
                context_stack.push(is_object);
                is_key = is_object;
            }
            '}' | ']' if !in_string => {
                result.push_str(gray);
                result.push(c);
                result.push_str(reset);
                context_stack.pop();
            }
            '0'..='9' | '-' | '.' if !in_string => {
                result.push_str(yellow);
                result.push(c);
                // Continue collecting the number
                while let Some(&next) = chars.peek() {
                    if next.is_ascii_digit() || next == '.' || next == 'e' || next == 'E' || next == '+' || next == '-' {
                        result.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                result.push_str(reset);
            }
            't' if !in_string => {
                // Check for "true"
                let rest: String = chars.by_ref().take(3).collect();
                if rest == "rue" {
                    result.push_str(yellow);
                    result.push_str("true");
                    result.push_str(reset);
                } else {
                    result.push('t');
                    result.push_str(&rest);
                }
            }
            'f' if !in_string => {
                // Check for "false"
                let rest: String = chars.by_ref().take(4).collect();
                if rest == "alse" {
                    result.push_str(yellow);
                    result.push_str("false");
                    result.push_str(reset);
                } else {
                    result.push('f');
                    result.push_str(&rest);
                }
            }
            'n' if !in_string => {
                // Check for "null"
                let rest: String = chars.by_ref().take(3).collect();
                if rest == "ull" {
                    result.push_str(yellow);
                    result.push_str("null");
                    result.push_str(reset);
                } else {
                    result.push('n');
                    result.push_str(&rest);
                }
            }
            _ => result.push(c),
        }
    }

    result
}

/// Colorize TOML output
fn colorize_toml(s: &str) -> String {
    let c = Colors::get();
    if c.cyan.is_empty() {
        return s.to_string();
    }

    let (green, cyan, magenta, yellow, gray, reset) =
        (c.green, c.cyan, c.magenta, c.yellow, c.gray, c.reset);

    let mut result = String::new();

    for line in s.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            // Section header
            result.push_str(green);
            result.push_str(line);
            result.push_str(reset);
        } else if let Some(eq_pos) = trimmed.find(" = ") {
            // Key = value
            let indent = &line[..line.len() - trimmed.len()];
            let key = &trimmed[..eq_pos];
            let value = &trimmed[eq_pos + 3..];

            result.push_str(indent);
            result.push_str(cyan);
            result.push_str(key);
            result.push_str(reset);
            result.push_str(gray);
            result.push_str(" = ");
            result.push_str(reset);
            result.push_str(&colorize_toml_value(value, magenta, yellow, gray, reset));
        } else {
            result.push_str(line);
        }
        result.push('\n');
    }

    result.trim_end().to_string()
}

fn colorize_toml_value(s: &str, magenta: &str, yellow: &str, gray: &str, reset: &str) -> String {
    let trimmed = s.trim();

    if trimmed.starts_with('"') && trimmed.ends_with('"') {
        format!("{}{}{}", magenta, trimmed, reset)
    } else if trimmed == "true" || trimmed == "false" || trimmed.parse::<f64>().is_ok() {
        format!("{}{}{}", yellow, trimmed, reset)
    } else if trimmed.starts_with('[') {
        // Array - colorize elements
        let mut result = format!("{}[{}", gray, reset);
        let inner = &trimmed[1..trimmed.len()-1];
        let parts: Vec<&str> = inner.split(", ").collect();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                result.push_str(&format!("{}, {}", gray, reset));
            }
            result.push_str(&colorize_toml_value(part, magenta, yellow, gray, reset));
        }
        result.push_str(&format!("{}]{}", gray, reset));
        result
    } else {
        s.to_string()
    }
}

/// Colorize YAML output
fn colorize_yaml(s: &str) -> String {
    let c = Colors::get();
    if c.cyan.is_empty() {
        return s.to_string();
    }

    let (cyan, magenta, yellow, gray, reset) = (c.cyan, c.magenta, c.yellow, c.gray, c.reset);

    let mut result = String::new();

    for line in s.lines() {
        if let Some(colon_pos) = line.find(':') {
            let before_colon = &line[..colon_pos];
            let after_colon = &line[colon_pos + 1..];

            // Check if this is a key (not a list item continuation)
            let trimmed_before = before_colon.trim_start_matches([' ', '-']);

            if !trimmed_before.is_empty() && !trimmed_before.starts_with('#') {
                let indent = &before_colon[..before_colon.len() - trimmed_before.len()];

                // Handle "- key:" pattern
                if indent.contains('-') {
                    let dash_pos = indent.find('-').unwrap();
                    result.push_str(&indent[..dash_pos]);
                    result.push_str(gray);
                    result.push('-');
                    result.push_str(reset);
                    result.push_str(&indent[dash_pos + 1..]);
                } else {
                    result.push_str(indent);
                }

                result.push_str(cyan);
                result.push_str(trimmed_before);
                result.push_str(reset);
                result.push_str(gray);
                result.push(':');
                result.push_str(reset);

                // Colorize value
                let value = after_colon.trim();
                if !value.is_empty() {
                    result.push(' ');
                    result.push_str(&colorize_yaml_value(value, magenta, yellow, reset));
                }
            } else {
                result.push_str(line);
            }
        } else if line.trim().starts_with('-') {
            // List item
            let trimmed = line.trim();
            let indent = &line[..line.len() - trimmed.len()];
            result.push_str(indent);
            result.push_str(gray);
            result.push('-');
            result.push_str(reset);
            result.push_str(&colorize_yaml_value(trimmed[1..].trim(), magenta, yellow, reset));
        } else {
            result.push_str(line);
        }
        result.push('\n');
    }

    result.trim_end().to_string()
}

fn colorize_yaml_value(s: &str, magenta: &str, yellow: &str, reset: &str) -> String {
    let trimmed = s.trim();

    if trimmed.starts_with('"') || trimmed.starts_with('\'') {
        format!("{}{}{}", magenta, trimmed, reset)
    } else if matches!(trimmed, "true" | "false" | "null" | "~") || trimmed.parse::<f64>().is_ok() {
        format!("{}{}{}", yellow, trimmed, reset)
    } else if !trimmed.is_empty() && !trimmed.contains(':') {
        // Unquoted string
        format!("{}{}{}", magenta, trimmed, reset)
    } else {
        s.to_string()
    }
}

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

        let c = Colors::get();
        let (gray, reset) = (c.gray, c.reset);

        // Try JSON first (most specific - must start with { or [)
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&unescaped) {
                if let Ok(pretty) = serde_json::to_string_pretty(&json) {
                    let colorized = colorize_json(&pretty);
                    raw_output = format!("{}(json){}\n{}", gray, reset, colorized);
                    return truncate_output(&raw_output);
                }
            }
        }

        // Try TOML (look for key = value or [section] patterns)
        if trimmed.contains(" = ") || trimmed.contains("]\n") || trimmed.starts_with('[') {
            if let Ok(toml_val) = toml::from_str::<toml::Value>(&unescaped) {
                if let Ok(pretty) = toml::to_string_pretty(&toml_val) {
                    let colorized = colorize_toml(&pretty);
                    raw_output = format!("{}(toml){}\n{}", gray, reset, colorized);
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
                        let colorized = colorize_yaml(pretty.trim());
                        raw_output = format!("{}(yaml){}\n{}", gray, reset, colorized);
                        return truncate_output(&raw_output);
                    }
                }
            }
        }

        // For plain text strings, show with newlines and word wrap
        raw_output = format!("{}(string, {} chars){}\n{}", gray, unescaped.len(), reset, word_wrap(&unescaped, 80));
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
    let c = Colors::get();
    if c.cyan.is_empty() {
        return s.to_string();
    }

    let (green, cyan, yellow, magenta, white, gray, reset) =
        (c.green, c.cyan, c.yellow, c.magenta, c.white, c.gray, c.reset);

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
            let name = trimmed.trim_end_matches(['{', '[', '(', ' ']);
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
            let name = trimmed.trim_end_matches(['{', '[', '(', ' ']);
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
        let c = Colors::get();
        let truncated = lines[..MAX_LINES].join("\n");
        format!("{}\n{}... ({} more lines){}", truncated, c.gray, lines.len() - MAX_LINES, c.reset)
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

/// Show help menu
fn show_help() {
    eprintln!("\n\x1b[1;33m─── print-break Help ───\x1b[0m");
    eprintln!("\x1b[36mEnter\x1b[0m     Continue to next breakpoint");
    eprintln!("\x1b[36mm\x1b[0m         Show full output (if truncated)");
    eprintln!("\x1b[36mt\x1b[0m         Show stack trace");
    eprintln!("\x1b[36mc\x1b[0m         Copy last value to clipboard");
    eprintln!("\x1b[36ms\x1b[0m         Skip all remaining breakpoints");
    eprintln!("\x1b[36mq\x1b[0m         Quit the program");
    eprintln!("\x1b[36mh / ?\x1b[0m     Show this help");
    eprintln!();
    eprintln!("\x1b[90mEnvironment variables:\x1b[0m");
    eprintln!("  \x1b[36mPRINT_BREAK=0\x1b[0m          Disable all breakpoints");
    eprintln!("  \x1b[36mPRINT_BREAK_DEPTH=N\x1b[0m    Max nesting depth (default: 4)");
    eprintln!("  \x1b[36mPRINT_BREAK_STYLE=X\x1b[0m    Border style: rounded, sharp, double, ascii");
    eprintln!("\x1b[1;33m─────────────────────────\x1b[0m\n");
}

/// Show stack trace
fn show_stack_trace() {
    eprintln!("\n\x1b[1;33m─── Stack Trace ───\x1b[0m");

    let bt = backtrace::Backtrace::new();
    let mut in_relevant = false;
    let mut count = 0;

    for frame in bt.frames() {
        for symbol in frame.symbols() {
            if let Some(name) = symbol.name() {
                let name_str = name.to_string();

                // Skip internal frames
                if name_str.contains("print_break::") || name_str.contains("backtrace::") {
                    continue;
                }

                // Start showing after we exit print_break internals
                if !in_relevant && !name_str.contains("print_break") {
                    in_relevant = true;
                }

                if in_relevant {
                    let file = symbol.filename()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default();
                    let line = symbol.lineno().unwrap_or(0);

                    // Simplify long paths
                    let short_file = file.rsplit('/').next().unwrap_or(&file);

                    if !name_str.contains("std::") && !name_str.contains("core::") && !name_str.contains("__rust") {
                        eprintln!("\x1b[90m{:>3}.\x1b[0m \x1b[36m{}\x1b[0m", count, name_str);
                        if !file.is_empty() && line > 0 {
                            eprintln!("      \x1b[90mat {}:{}\x1b[0m", short_file, line);
                        }
                        count += 1;

                        if count >= 15 {
                            eprintln!("\x1b[90m     ... (truncated)\x1b[0m");
                            break;
                        }
                    }
                }
            }
        }
        if count >= 15 {
            break;
        }
    }

    eprintln!("\x1b[1;33m───────────────────\x1b[0m\n");
}

/// Copy text to clipboard using system commands
fn copy_to_clipboard(text: &str) -> bool {
    use std::process::{Command, Stdio};
    use std::io::Write as IoWrite;

    // Try different clipboard commands based on platform
    let commands = if cfg!(target_os = "macos") {
        vec![("pbcopy", vec![])]
    } else if cfg!(target_os = "windows") {
        vec![("clip", vec![])]
    } else {
        // Linux - try multiple options
        vec![
            ("xclip", vec!["-selection", "clipboard"]),
            ("xsel", vec!["--clipboard", "--input"]),
            ("wl-copy", vec![]),
        ]
    };

    for (cmd, args) in commands {
        if let Ok(mut child) = Command::new(cmd)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                if stdin.write_all(text.as_bytes()).is_ok() {
                    drop(stdin);
                    if child.wait().map(|s| s.success()).unwrap_or(false) {
                        return true;
                    }
                }
            }
        }
    }
    false
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
        eprint!("\x1b[90m[Enter, m=more, t=trace, c=copy, s=skip, q=quit, h=help]\x1b[0m ");
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
                    continue;
                }
                "t" | "trace" => {
                    show_stack_trace();
                    continue;
                }
                "c" | "copy" => {
                    if let Ok(guard) = LAST_FULL_OUTPUT.lock() {
                        if let Some(ref full) = *guard {
                            // Strip ANSI codes for clipboard
                            let clean = strip_ansi_codes(full);
                            if copy_to_clipboard(&clean) {
                                eprintln!("\x1b[1;32mCopied to clipboard!\x1b[0m");
                            } else {
                                eprintln!("\x1b[1;31mFailed to copy (install xclip or xsel)\x1b[0m");
                            }
                        } else {
                            eprintln!("\x1b[90m(nothing to copy)\x1b[0m");
                        }
                    }
                    continue;
                }
                "h" | "?" | "help" => {
                    show_help();
                    continue;
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

/// Strip ANSI escape codes from a string
fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::new();
    let mut in_escape = false;

    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else {
            result.push(c);
        }
    }

    result
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
            let break_id = $crate::next_break_id();
            let elapsed_str = $crate::get_elapsed().map($crate::format_elapsed).unwrap_or_default();
            $crate::update_break_time();

            let location = format!("{}:{}", file!(), line!());
            let width = 50;
            let border = $crate::get_border_style();
            let c = $crate::Colors::get();

            let h = border.horizontal.to_string();

            eprintln!();
            eprintln!("{}{}{} BREAK #{} {}{}{}", c.yellow, border.top_left, h, break_id, elapsed_str, h.repeat(width - 14 - break_id.to_string().len() - elapsed_str.len() / 3), c.reset);
            eprintln!("{}{}{} {}{}{}", c.yellow, border.vertical, c.reset, c.cyan, location, c.reset);
            eprintln!("{}{}{}{}", c.yellow, border.bottom_left, h.repeat(width), c.reset);

            $crate::handle_input();
        }
    }};
    ($($var:expr),+ $(,)?) => {{
        if $crate::is_enabled() {
            let break_id = $crate::next_break_id();
            let elapsed_str = $crate::get_elapsed().map($crate::format_elapsed).unwrap_or_default();
            $crate::update_break_time();

            let location = format!("{}:{}", file!(), line!());
            let width = 50;
            let border = $crate::get_border_style();
            let c = $crate::Colors::get();

            // Collect full output for "more" option
            let mut full_output = String::new();

            let h = border.horizontal.to_string();

            eprintln!();
            eprintln!("{}{}{} BREAK #{} {}{}{}", c.yellow, border.top_left, h, break_id, elapsed_str, h.repeat(width - 14 - break_id.to_string().len() - elapsed_str.len() / 3), c.reset);
            eprintln!("{}{}{} {}{}{}", c.yellow, border.vertical, c.reset, c.cyan, location, c.reset);
            eprintln!("{}{}{}{}", c.yellow, border.tee_right, h.repeat(width), c.reset);

            $(
                let formatted = $crate::format_value(&$var);
                let name = stringify!($var);

                // Store untruncated version
                full_output.push_str(&format!("{} = {}\n\n", name, $crate::format_value_full(&$var)));

                if formatted.contains('\n') {
                    eprintln!("{}{}{} {}{}{}=", c.yellow, border.vertical, c.reset, c.green, name, c.reset);
                    for line in formatted.lines() {
                        eprintln!("{}{}{} {}{}{}", c.yellow, border.vertical, c.reset, c.white, line, c.reset);
                    }
                } else {
                    eprintln!("{}{}{} {}{}{} = {}{}{}", c.yellow, border.vertical, c.reset, c.green, name, c.reset, c.white, formatted, c.reset);
                }
            )+

            $crate::store_full_output(full_output);

            eprintln!("{}{}{}{}", c.yellow, border.bottom_left, h.repeat(width), c.reset);
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
