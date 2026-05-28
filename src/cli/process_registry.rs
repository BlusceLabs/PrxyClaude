use std::collections::HashSet;
use std::sync::Mutex;

static PIDS: std::sync::LazyLock<Mutex<HashSet<u32>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashSet::new()));

pub fn register_pid(pid: u32) {
    if pid == 0 {
        return;
    }
    if let Ok(mut pids) = PIDS.lock() {
        pids.insert(pid);
    }
}

pub fn unregister_pid(pid: u32) {
    if pid == 0 {
        return;
    }
    if let Ok(mut pids) = PIDS.lock() {
        pids.remove(&pid);
    }
}

pub fn kill_all_best_effort() {
    let pids: Vec<u32> = {
        if let Ok(mut pids) = PIDS.lock() {
            let pids_vec: Vec<u32> = pids.iter().copied().collect();
            pids.clear();
            pids_vec
        } else {
            return;
        }
    };

    if pids.is_empty() {
        return;
    }

    #[cfg(unix)]
    {
        for pid in pids {
            let _ = std::process::Command::new("kill")
                .args(["-9", &pid.to_string()])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
    }
    #[cfg(windows)]
    {
        for pid in pids {
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_unregister_pid() {
        register_pid(12345);
        register_pid(0);
        {
            let pids = PIDS.lock().unwrap();
            assert!(pids.contains(&12345));
            assert!(!pids.contains(&0));
        }
        unregister_pid(12345);
        {
            let pids = PIDS.lock().unwrap();
            assert!(!pids.contains(&12345));
        }
    }

    #[test]
    fn test_register_zero_pid() {
        register_pid(0);
        let pids = PIDS.lock().unwrap();
        assert!(!pids.contains(&0));
    }
}
