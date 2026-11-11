use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

static PATH_LOCK: Mutex<()> = Mutex::new(());

pub struct FakeTools {
    directory: PathBuf,
    original_path: Option<OsString>,
    _guard: MutexGuard<'static, ()>,
}

impl FakeTools {
    pub fn install(tools: &[(&str, &str)]) -> Self {
        let guard = PATH_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let directory = std::env::temp_dir().join(format!(
            "csfx-fake-tools-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        for (name, behaviour) in tools {
            let log = directory.join(format!("{name}.log"));
            let script = format!(
                "#!/bin/sh\necho \"$*\" >> '{}'\n{behaviour}\n",
                log.display()
            );
            let path = directory.join(name);
            std::fs::write(&path, script).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let original_path = std::env::var_os("PATH");
        let mut search = OsString::from(&directory);
        if let Some(original) = &original_path {
            search.push(":");
            search.push(original);
        }
        std::env::set_var("PATH", search);
        Self {
            directory,
            original_path,
            _guard: guard,
        }
    }

    pub fn calls(&self, tool: &str) -> Vec<String> {
        std::fs::read_to_string(self.directory.join(format!("{tool}.log")))
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }
}

impl Drop for FakeTools {
    fn drop(&mut self) {
        match &self.original_path {
            Some(original) => std::env::set_var("PATH", original),
            None => std::env::remove_var("PATH"),
        }
        std::fs::remove_dir_all(&self.directory).ok();
    }
}
