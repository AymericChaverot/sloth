//! Helpers for tests that need a real git repository on disk.

use std::path::PathBuf;
use std::process::Command;

pub struct TempRepo {
    pub path: PathBuf,
    counter: std::cell::Cell<u32>,
    /// Only the outermost repository deletes its directory.
    owns_dir: bool,
}

impl TempRepo {
    /// Creates an empty repository with `main` as its initial branch in a
    /// fresh temporary directory, removed on drop.
    pub fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "sloth-test-{}-{}-{}",
            name,
            std::process::id(),
            crate::git::stats::now_ts()
        ));
        let _ = std::fs::remove_dir_all(&path);
        let mut repo = Self::at(path);
        repo.owns_dir = true;
        repo
    }

    /// Creates an empty repository at `path` (e.g. nested in another one).
    pub fn at(path: PathBuf) -> Self {
        std::fs::create_dir_all(&path).expect("create temp repo dir");
        let repo = Self {
            path,
            counter: std::cell::Cell::new(0),
            owns_dir: false,
        };
        repo.git(&["init", "-q", "-b", "main"]);
        repo
    }

    pub fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args([
                "-c",
                "user.name=Sloth Test",
                "-c",
                "user.email=test@sloth.invalid",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(&self.path)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    /// Writes `content` to `file` and commits it.
    pub fn commit(&self, file: &str, content: &str) {
        std::fs::write(self.path.join(file), content).expect("write file");
        self.git(&["add", file]);
        let n = self.counter.get() + 1;
        self.counter.set(n);
        self.git(&["commit", "-q", "-m", &format!("commit {n}")]);
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        if self.owns_dir {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}
