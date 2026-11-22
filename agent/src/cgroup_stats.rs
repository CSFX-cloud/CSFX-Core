use std::path::{Path, PathBuf};
use tokio::sync::Mutex;
use tracing::warn;

const CGROUP_ROOT: &str = "/sys/fs/cgroup";

pub fn service_cgroup_path(unit_name: &str) -> PathBuf {
    Path::new(CGROUP_ROOT)
        .join("system.slice")
        .join(format!("{unit_name}.service"))
}

pub async fn read_cpu_usage_percent(
    cgroup_path: &Path,
    previous_sample: &Mutex<Option<(std::time::Instant, u64)>>,
) -> Option<f64> {
    let usage_usec = read_cgroup_u64(cgroup_path, "cpu.stat", "usage_usec").await?;
    let now = std::time::Instant::now();

    let mut previous = previous_sample.lock().await;
    let percent = match *previous {
        Some((previous_time, previous_usage)) => {
            let elapsed_usec = now.duration_since(previous_time).as_micros() as u64;
            if elapsed_usec == 0 || usage_usec < previous_usage {
                None
            } else {
                let delta_usec = usage_usec - previous_usage;
                Some((delta_usec as f64 / elapsed_usec as f64) * 100.0)
            }
        }
        None => None,
    };

    *previous = Some((now, usage_usec));
    percent
}

pub async fn read_memory_usage_bytes(cgroup_path: &Path) -> Option<i64> {
    let path = cgroup_path.join("memory.current");
    match tokio::fs::read_to_string(&path).await {
        Ok(content) => content.trim().parse::<i64>().ok(),
        Err(e) => {
            warn!(path = %path.display(), error = %e, "failed to read workload memory cgroup");
            None
        }
    }
}

pub async fn read_cgroup_u64(cgroup_path: &Path, file: &str, key: &str) -> Option<u64> {
    let path = cgroup_path.join(file);
    let content = match tokio::fs::read_to_string(&path).await {
        Ok(content) => content,
        Err(e) => {
            warn!(path = %path.display(), error = %e, "failed to read workload cgroup file");
            return None;
        }
    };

    content.lines().find_map(|line| {
        let (line_key, value) = line.split_once(' ')?;
        if line_key == key {
            value.trim().parse::<u64>().ok()
        } else {
            None
        }
    })
}

pub async fn read_memory_working_set_bytes(cgroup_path: &Path) -> Option<i64> {
    let current = read_memory_usage_bytes(cgroup_path).await?;
    let reclaimable = read_cgroup_u64(cgroup_path, "memory.stat", "inactive_file")
        .await
        .unwrap_or(0) as i64;
    Some((current - reclaimable).max(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn working_set_excludes_inactive_page_cache() {
        let dir = std::env::temp_dir().join(format!("csfx-cgroup-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("memory.current"), "1000\n").unwrap();
        std::fs::write(dir.join("memory.stat"), "anon 600\ninactive_file 300\n").unwrap();
        assert_eq!(read_memory_working_set_bytes(&dir).await, Some(700));
        std::fs::write(dir.join("memory.stat"), "anon 600\n").unwrap();
        assert_eq!(read_memory_working_set_bytes(&dir).await, Some(1000));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
