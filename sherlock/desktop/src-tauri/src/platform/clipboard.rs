#[cfg(target_os = "linux")]
use std::io::Write;
#[cfg(target_os = "linux")]
use std::process::{Command, Stdio};

/// Copy text to the system clipboard.
///
/// On Linux, tries native clipboard tools (wl-copy, xclip, xsel) first
/// for better persistence, falling back to arboard.
/// On macOS/Windows, uses arboard directly.
pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        // wl-copy (Wayland)
        if try_pipe_to_clipboard("wl-copy", &[], text) {
            return Ok(());
        }
        // xclip (X11)
        if try_pipe_to_clipboard("xclip", &["-selection", "clipboard"], text) {
            return Ok(());
        }
        // xsel (X11 alternative)
        if try_pipe_to_clipboard("xsel", &["--clipboard", "--input"], text) {
            return Ok(());
        }
    }

    // macOS/Windows or Linux fallback: arboard
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(text).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn try_pipe_to_clipboard(cmd: &str, args: &[&str], text: &str) -> bool {
    let Ok(mut child) = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    if let Some(mut stdin) = child.stdin.take() {
        if stdin.write_all(text.as_bytes()).is_err() {
            return false;
        }
    }
    child.wait().is_ok_and(|s| s.success())
}

#[cfg(test)]
mod tests {
    #[test]
    fn copy_to_clipboard_compiles() {
        // Smoke test: just verify the function signature is correct.
        // Actual clipboard tests require a display server.
        let _ = super::copy_to_clipboard;
    }
}
