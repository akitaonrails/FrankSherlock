use std::process::Command;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub vram_used_mib: Option<u64>,
    pub vram_total_mib: Option<u64>,
}

/// Detect GPU VRAM usage via platform-appropriate tools.
///
/// - Linux/Windows: tries `nvidia-smi`
/// - macOS: returns `None` (Apple Silicon has unified memory)
pub fn detect_gpu_memory() -> GpuInfo {
    #[cfg(target_os = "macos")]
    {
        GpuInfo {
            vram_used_mib: None,
            vram_total_mib: None,
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Ok(output) = Command::new("nvidia-smi")
            .args([
                "--query-gpu=memory.used,memory.total",
                "--format=csv,noheader,nounits",
            ])
            .output()
        {
            if output.status.success() {
                let (used, total) =
                    parse_nvidia_smi_output(&String::from_utf8_lossy(&output.stdout));
                return GpuInfo {
                    vram_used_mib: used,
                    vram_total_mib: total,
                };
            }
        }
        GpuInfo {
            vram_used_mib: None,
            vram_total_mib: None,
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn parse_nvidia_smi_output(text: &str) -> (Option<u64>, Option<u64>) {
    let Some(first_line) = text.lines().next() else {
        return (None, None);
    };
    let mut parts = first_line.split(',');
    let used = parts.next().and_then(|v| v.trim().parse::<u64>().ok());
    let total = parts.next().and_then(|v| v.trim().parse::<u64>().ok());
    (used, total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn parses_nvidia_memory() {
        let sample = "1024, 24564\n";
        let (used, total) = parse_nvidia_smi_output(sample);
        assert_eq!(used, Some(1024));
        assert_eq!(total, Some(24564));
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn parses_nvidia_empty() {
        let (used, total) = parse_nvidia_smi_output("");
        assert!(used.is_none());
        assert!(total.is_none());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_returns_none() {
        let info = detect_gpu_memory();
        assert!(info.vram_used_mib.is_none());
        assert!(info.vram_total_mib.is_none());
    }
}
