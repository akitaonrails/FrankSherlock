use std::path::PathBuf;

/// Find an executable by name on the system PATH.
///
/// Uses the `which` crate for cross-platform lookup
/// (handles PATHEXT on Windows automatically).
#[allow(dead_code)]
pub fn find_executable(name: &str) -> Option<PathBuf> {
    which::which(name).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_executable_known() {
        // Every OS has some basic executable we can test with
        #[cfg(target_os = "windows")]
        let name = "cmd";
        #[cfg(not(target_os = "windows"))]
        let name = "sh";

        let result = find_executable(name);
        assert!(result.is_some(), "should find '{name}' on PATH");
    }

    #[test]
    fn find_executable_nonexistent() {
        let result = find_executable("this_executable_does_not_exist_xyz123");
        assert!(result.is_none());
    }
}
