use std::io::Write;
use std::process::Command;

/// Trait for clipboard read/write operations.
pub trait ClipboardProvider {
    /// Write text to the system clipboard.
    fn write(&self, text: &str) -> Result<(), String>;
    /// Read text from the system clipboard.
    fn read(&self) -> Result<String, String>;
    /// Human-readable name for this provider.
    #[allow(dead_code)]
    fn name(&self) -> &str;
}

/// OSC 52 clipboard writer -- works over SSH and in most modern terminals.
pub struct Osc52Writer {
    in_tmux: bool,
}

impl Osc52Writer {
    fn write_osc52(&self, text: &str) -> Result<(), String> {
        let encoded = base64_encode(text.as_bytes());
        let mut stdout = std::io::stdout();

        if self.in_tmux {
            // Wrap in DCS passthrough for tmux
            write!(stdout, "\x1bPtmux;\x1b\x1b]52;c;{}\x07\x1b\\", encoded)
                .map_err(|e| format!("OSC 52 tmux write failed: {}", e))?;
        } else {
            write!(stdout, "\x1b]52;c;{}\x07", encoded)
                .map_err(|e| format!("OSC 52 write failed: {}", e))?;
        }

        stdout.flush().map_err(|e| format!("flush failed: {}", e))?;
        Ok(())
    }
}

/// Platform-native clipboard tool.
#[derive(Debug, Clone)]
pub enum NativeProvider {
    MacOs,
    Wayland,
    Xclip,
    Xsel,
}

impl NativeProvider {
    fn write_native(&self, text: &str) -> Result<(), String> {
        let mut child = match self {
            NativeProvider::MacOs => Command::new("pbcopy")
                .stdin(std::process::Stdio::piped())
                .spawn(),
            NativeProvider::Wayland => Command::new("wl-copy")
                .stdin(std::process::Stdio::piped())
                .spawn(),
            NativeProvider::Xclip => Command::new("xclip")
                .args(["-selection", "clipboard"])
                .stdin(std::process::Stdio::piped())
                .spawn(),
            NativeProvider::Xsel => Command::new("xsel")
                .args(["--clipboard", "--input"])
                .stdin(std::process::Stdio::piped())
                .spawn(),
        }
        .map_err(|e| format!("Failed to spawn clipboard tool: {}", e))?;

        if let Some(ref mut stdin) = child.stdin {
            stdin
                .write_all(text.as_bytes())
                .map_err(|e| format!("Failed to write to clipboard tool: {}", e))?;
        }

        child
            .wait()
            .map_err(|e| format!("Clipboard tool failed: {}", e))?;
        Ok(())
    }

    fn read_native(&self) -> Result<String, String> {
        let output = match self {
            NativeProvider::MacOs => Command::new("pbpaste").output(),
            NativeProvider::Wayland => Command::new("wl-paste").output(),
            NativeProvider::Xclip => Command::new("xclip")
                .args(["-selection", "clipboard", "-o"])
                .output(),
            NativeProvider::Xsel => Command::new("xsel")
                .args(["--clipboard", "--output"])
                .output(),
        }
        .map_err(|e| format!("Failed to read clipboard: {}", e))?;

        if output.status.success() {
            String::from_utf8(output.stdout)
                .map_err(|e| format!("Clipboard contained invalid UTF-8: {}", e))
        } else {
            Err("Clipboard read returned non-zero exit".to_string())
        }
    }
}

/// Composite provider: OSC 52 primary, platform-native fallback.
pub struct CompositeProvider {
    writer: Osc52Writer,
    reader: Option<NativeProvider>,
}

impl ClipboardProvider for CompositeProvider {
    fn write(&self, text: &str) -> Result<(), String> {
        // OSC 52 first (primary -- works over SSH)
        let osc_result = self.writer.write_osc52(text);

        // Best-effort native write as fallback
        if let Some(ref native) = self.reader {
            let _ = native.write_native(text);
        }

        osc_result
    }

    fn read(&self) -> Result<String, String> {
        if let Some(ref native) = self.reader {
            native.read_native()
        } else {
            Err("No native clipboard tool available for reading (OSC 52 is write-only)".to_string())
        }
    }

    fn name(&self) -> &str {
        match &self.reader {
            Some(NativeProvider::MacOs) => "OSC 52 + pbcopy",
            Some(NativeProvider::Wayland) => "OSC 52 + wl-copy",
            Some(NativeProvider::Xclip) => "OSC 52 + xclip",
            Some(NativeProvider::Xsel) => "OSC 52 + xsel",
            None => "OSC 52 only",
        }
    }
}

/// Detect the best clipboard provider for this platform.
/// Called once at startup.
pub fn detect_provider() -> Box<dyn ClipboardProvider> {
    let in_tmux = std::env::var("TMUX").is_ok();

    let native = if cfg!(target_os = "macos") {
        Some(NativeProvider::MacOs)
    } else if std::env::var("WAYLAND_DISPLAY").is_ok() && command_exists("wl-copy") {
        Some(NativeProvider::Wayland)
    } else if std::env::var("DISPLAY").is_ok() {
        if command_exists("xclip") {
            Some(NativeProvider::Xclip)
        } else if command_exists("xsel") {
            Some(NativeProvider::Xsel)
        } else {
            None
        }
    } else {
        None
    };

    Box::new(CompositeProvider {
        writer: Osc52Writer { in_tmux },
        reader: native,
    })
}

/// Check if a command exists on the system PATH.
fn command_exists(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Inline base64 encoder (no external crate).
/// Standard base64 encoding with no line breaks.
fn base64_encode(input: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut result = String::with_capacity((input.len() + 2) / 3 * 4);
    let chunks = input.chunks(3);

    for chunk in chunks {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };

        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_encode_empty() {
        assert_eq!(base64_encode(b""), "");
    }

    #[test]
    fn test_base64_encode_hello() {
        assert_eq!(base64_encode(b"Hello, World!"), "SGVsbG8sIFdvcmxkIQ==");
    }

    #[test]
    fn test_base64_encode_no_padding() {
        assert_eq!(base64_encode(b"abc"), "YWJj");
    }

    #[test]
    fn test_base64_encode_one_pad() {
        assert_eq!(base64_encode(b"ab"), "YWI=");
    }
}
