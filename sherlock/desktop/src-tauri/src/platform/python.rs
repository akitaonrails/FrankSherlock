use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PythonStatus {
    pub available: bool,
    pub version: Option<String>,
    pub venv_exists: bool,
}

/// Returns the path to the Python binary inside a virtual environment.
/// On Unix: `venv/bin/python`
/// On Windows: `venv/Scripts/python.exe`
pub fn python_venv_binary(venv: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        venv.join("Scripts").join("python.exe")
    }
    #[cfg(not(target_os = "windows"))]
    {
        venv.join("bin").join("python")
    }
}

/// Check whether a Python venv exists and the interpreter is runnable.
pub fn check_python_available(venv: &Path) -> PythonStatus {
    let venv_exists = venv.exists();
    let python_bin = python_venv_binary(venv);

    if !venv_exists || !python_bin.exists() {
        return PythonStatus {
            available: false,
            version: None,
            venv_exists,
        };
    }

    match Command::new(&python_bin).arg("--version").output() {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let version = stdout
                .trim()
                .strip_prefix("Python ")
                .unwrap_or(stdout.trim())
                .to_string();
            PythonStatus {
                available: true,
                version: Some(version),
                venv_exists,
            }
        }
        _ => PythonStatus {
            available: false,
            version: None,
            venv_exists,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_venv_binary_unix_path() {
        let venv = Path::new("/home/user/.local/share/frank_sherlock/surya_venv");
        let bin = python_venv_binary(venv);

        #[cfg(target_os = "windows")]
        assert_eq!(
            bin,
            PathBuf::from("/home/user/.local/share/frank_sherlock/surya_venv/Scripts/python.exe")
        );
        #[cfg(not(target_os = "windows"))]
        assert_eq!(
            bin,
            PathBuf::from("/home/user/.local/share/frank_sherlock/surya_venv/bin/python")
        );
    }

    #[test]
    fn check_python_nonexistent_venv() {
        let status = check_python_available(Path::new("/nonexistent/venv/path"));
        assert!(!status.available);
        assert!(status.version.is_none());
        assert!(!status.venv_exists);
    }
}
