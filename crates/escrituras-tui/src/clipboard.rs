//! Cross-platform clipboard copy.
//!
//! Tries the platform's native clipboard tool first (`pbcopy` on macOS,
//! `clip` on Windows, `wl-copy`/`xclip`/`xsel` on Linux and other Unixes),
//! then falls back to the OSC 52 terminal escape sequence. OSC 52 asks the
//! terminal emulator to set the clipboard, so it also works over SSH and on
//! headless machines in terminals that support it (iTerm2, Windows Terminal,
//! kitty, WezTerm, Alacritty, tmux with `set-clipboard on`).
//!
//! Native tools are skipped inside SSH sessions, where they would set the
//! remote machine's clipboard instead of the user's.

use std::io::{IsTerminal, Write};
use std::process::{Command, Stdio};

/// Copy `text` to the system clipboard.
///
/// Returns an error only if no method could be attempted. OSC 52 gives no
/// feedback, so a terminal that ignores it still counts as success.
pub fn copy(text: &str) -> Result<(), String> {
    let env = |key: &str| std::env::var_os(key).is_some_and(|v| !v.is_empty());

    if !env("SSH_CONNECTION") && !env("SSH_TTY") {
        for tool in native_tools(env) {
            if run_tool(&tool, text) {
                return Ok(());
            }
        }
    }

    write_osc52(text).map_err(|e| format!("could not write to terminal: {}", e))
}

/// A native clipboard command that reads the text from stdin.
#[derive(Debug, PartialEq)]
struct Tool {
    program: &'static str,
    args: &'static [&'static str],
    /// Windows `clip` expects UTF-16LE with a BOM to preserve non-ASCII text.
    utf16: bool,
}

const fn tool(program: &'static str, args: &'static [&'static str]) -> Tool {
    Tool {
        program,
        args,
        utf16: false,
    }
}

/// Native clipboard tools to try on this platform, in order.
/// `env` reports whether an environment variable is set and non-empty.
fn native_tools(env: impl Fn(&str) -> bool) -> Vec<Tool> {
    if cfg!(target_os = "macos") {
        return vec![tool("pbcopy", &[])];
    }
    if cfg!(windows) {
        return vec![Tool {
            utf16: true,
            ..tool("clip", &[])
        }];
    }

    let mut tools = Vec::new();
    if env("WAYLAND_DISPLAY") {
        tools.push(tool("wl-copy", &[]));
    }
    if env("DISPLAY") {
        tools.push(tool("xclip", &["-selection", "clipboard"]));
        tools.push(tool("xsel", &["--clipboard", "--input"]));
    }
    if env("WSL_DISTRO_NAME") {
        tools.push(Tool {
            utf16: true,
            ..tool("clip.exe", &[])
        });
    }
    tools
}

/// Run a clipboard tool with `text` on stdin. Returns true on success.
fn run_tool(tool: &Tool, text: &str) -> bool {
    // stdout/stderr must not reach the TUI; xclip and wl-copy also fork a
    // background process that would otherwise keep writing to the terminal.
    let Ok(mut child) = Command::new(tool.program)
        .args(tool.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };

    let input = if tool.utf16 {
        utf16le_with_bom(text)
    } else {
        text.as_bytes().to_vec()
    };
    // Dropping stdin closes the pipe so the tool sees EOF.
    let wrote = child
        .stdin
        .take()
        .is_some_and(|mut stdin| stdin.write_all(&input).is_ok());

    let succeeded = child.wait().is_ok_and(|status| status.success());
    wrote && succeeded
}

fn utf16le_with_bom(text: &str) -> Vec<u8> {
    std::iter::once(0xFEFF)
        .chain(text.encode_utf16())
        .flat_map(u16::to_le_bytes)
        .collect()
}

/// Write the OSC 52 sequence to stderr, the stream the TUI draws on (see
/// `tui::init`), so it reaches the terminal even if stdout is redirected.
fn write_osc52(text: &str) -> std::io::Result<()> {
    let mut stderr = std::io::stderr().lock();
    if !stderr.is_terminal() {
        return Err(std::io::Error::other("stderr is not a terminal"));
    }
    stderr.write_all(osc52_sequence(text).as_bytes())?;
    stderr.flush()
}

fn osc52_sequence(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", base64_encode(text.as_bytes()))
}

fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i) & 0x3f) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_encode_rfc4648_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn test_base64_encode_utf8_and_high_bytes() {
        assert_eq!(base64_encode("é".as_bytes()), "w6k=");
        assert_eq!(base64_encode(&[0xff, 0xfe, 0xfd]), "//79");
    }

    #[test]
    fn test_osc52_sequence() {
        assert_eq!(osc52_sequence("foobar"), "\x1b]52;c;Zm9vYmFy\x07");
    }

    #[test]
    fn test_utf16le_with_bom() {
        assert_eq!(
            utf16le_with_bom("Aé"),
            vec![0xff, 0xfe, 0x41, 0x00, 0xe9, 0x00]
        );
    }

    #[test]
    fn test_native_tools_for_platform() {
        let set = |vars: &'static [&'static str]| move |key: &str| vars.contains(&key);

        if cfg!(target_os = "macos") {
            assert_eq!(native_tools(set(&[])), vec![tool("pbcopy", &[])]);
        } else if cfg!(windows) {
            let tools = native_tools(set(&[]));
            assert_eq!(tools.len(), 1);
            assert_eq!(tools[0].program, "clip");
            assert!(tools[0].utf16);
        } else {
            // Headless: nothing native, OSC 52 only
            assert!(native_tools(set(&[])).is_empty());

            let names = |tools: Vec<Tool>| tools.iter().map(|t| t.program).collect::<Vec<_>>();
            assert_eq!(names(native_tools(set(&["WAYLAND_DISPLAY"]))), ["wl-copy"]);
            assert_eq!(names(native_tools(set(&["DISPLAY"]))), ["xclip", "xsel"]);
            assert_eq!(
                names(native_tools(set(&["WAYLAND_DISPLAY", "DISPLAY"]))),
                ["wl-copy", "xclip", "xsel"]
            );
            assert_eq!(names(native_tools(set(&["WSL_DISTRO_NAME"]))), ["clip.exe"]);
        }
    }
}
