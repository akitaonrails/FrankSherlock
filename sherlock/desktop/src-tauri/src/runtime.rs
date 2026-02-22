use std::process::Command;

use crate::models::RuntimeStatus;

pub fn gather_runtime_status() -> RuntimeStatus {
    let mut loaded_models = Vec::new();
    let mut ollama_available = false;

    if let Ok(output) = Command::new("ollama").arg("ps").output() {
        if output.status.success() {
            ollama_available = true;
            loaded_models = parse_ollama_ps_output(&String::from_utf8_lossy(&output.stdout));
        }
    }

    let (vram_used_mib, vram_total_mib) = if let Ok(output) = Command::new("nvidia-smi")
        .args([
            "--query-gpu=memory.used,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        if output.status.success() {
            parse_nvidia_smi_output(&String::from_utf8_lossy(&output.stdout))
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    RuntimeStatus {
        current_model: loaded_models.first().cloned(),
        loaded_models,
        vram_used_mib,
        vram_total_mib,
        ollama_available,
    }
}

pub fn list_installed_ollama_models() -> Option<Vec<String>> {
    let output = Command::new("ollama").arg("list").output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(parse_ollama_list_output(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

fn parse_ollama_ps_output(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines().skip(1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(model) = trimmed.split_whitespace().next() {
            out.push(model.to_string());
        }
    }
    out
}

fn parse_nvidia_smi_output(text: &str) -> (Option<u64>, Option<u64>) {
    let Some(first_line) = text.lines().next() else {
        return (None, None);
    };
    let mut parts = first_line.split(',');
    let used = parts.next().and_then(|v| v.trim().parse::<u64>().ok());
    let total = parts.next().and_then(|v| v.trim().parse::<u64>().ok());
    (used, total)
}

fn parse_ollama_list_output(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines().skip(1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(name) = trimmed.split_whitespace().next() {
            out.push(name.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ollama_ps_rows() {
        let sample = "NAME ID SIZE PROCESSOR UNTIL\nqwen2.5vl:7b abc 6.0 GB 100% GPU 4 minutes\n";
        let models = parse_ollama_ps_output(sample);
        assert_eq!(models, vec!["qwen2.5vl:7b".to_string()]);
    }

    #[test]
    fn parses_nvidia_memory() {
        let sample = "1024, 24564\n";
        let (used, total) = parse_nvidia_smi_output(sample);
        assert_eq!(used, Some(1024));
        assert_eq!(total, Some(24564));
    }

    #[test]
    fn parses_ollama_list_rows() {
        let sample = "NAME ID SIZE MODIFIED\nqwen2.5vl:7b abc 5 GB 1 day ago\n";
        let models = parse_ollama_list_output(sample);
        assert_eq!(models, vec!["qwen2.5vl:7b".to_string()]);
    }
}
