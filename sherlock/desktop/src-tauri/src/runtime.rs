use std::process::Command;

use crate::models::RuntimeStatus;

pub fn gather_runtime_status() -> RuntimeStatus {
    let mut loaded_models = Vec::new();
    let mut ollama_available = false;

    if let Ok(output) = Command::new("ollama").arg("ps").output() {
        if output.status.success() {
            ollama_available = true;
            loaded_models = parse_ollama_table_output(&String::from_utf8_lossy(&output.stdout));
        }
    }

    let gpu = crate::platform::gpu::detect_gpu_memory();

    RuntimeStatus {
        current_model: loaded_models.first().cloned(),
        loaded_models,
        vram_used_mib: gpu.vram_used_mib,
        vram_total_mib: gpu.vram_total_mib,
        ollama_available,
    }
}

pub fn list_installed_ollama_models() -> Option<Vec<String>> {
    let output = Command::new("ollama").arg("list").output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(parse_ollama_table_output(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

/// Parse the first whitespace-delimited column from each non-header line.
/// Works for both `ollama ps` and `ollama list` output.
fn parse_ollama_table_output(text: &str) -> Vec<String> {
    text.lines()
        .skip(1) // skip header row
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .filter_map(|l| l.split_whitespace().next())
        .map(|s| s.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ollama_ps_rows() {
        let sample = "NAME ID SIZE PROCESSOR UNTIL\nqwen2.5vl:7b abc 6.0 GB 100% GPU 4 minutes\n";
        let models = parse_ollama_table_output(sample);
        assert_eq!(models, vec!["qwen2.5vl:7b".to_string()]);
    }

    #[test]
    fn parses_ollama_list_rows() {
        let sample = "NAME ID SIZE MODIFIED\nqwen2.5vl:7b abc 5 GB 1 day ago\n";
        let models = parse_ollama_table_output(sample);
        assert_eq!(models, vec!["qwen2.5vl:7b".to_string()]);
    }
}
