use std::path::Path;

pub trait FileSystem {
    fn get_size(&self, path: &Path) -> std::io::Result<u64>;
    /// Whether `path` is the root of a Git repository (or worktree).
    fn is_repository(&self, path: &Path) -> bool;
}

pub trait GitExecutor {
    fn run_git_command(&self, path: &Path, args: &[&str]) -> std::io::Result<String>;
    fn run_git_command_async(
        &self,
        path: &Path,
        args: &[&str],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = std::io::Result<String>> + Send + '_>>;
    fn open_repo(&self, path: &Path) -> Result<(), crate::git::models::GitError>;
    /// Runs a git command feeding `input` on its stdin.
    fn run_git_command_with_input(
        &self,
        path: &Path,
        args: &[&str],
        input: &str,
    ) -> std::io::Result<String>;
}

/// A `git` process with a stable, non-interactive environment: output is
/// parsed, so it must not be translated, and nothing may prompt for input.
fn git_command() -> std::process::Command {
    let mut cmd = std::process::Command::new("git");
    cmd.env("LC_ALL", "C")
        .env("LANGUAGE", "C")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_PAGER", "cat")
        .stdin(std::process::Stdio::null());
    cmd
}

#[derive(Clone)]
pub struct RealSystem;

impl FileSystem for RealSystem {
    /// Apparent size of a file or directory tree. Symlinks (and Windows
    /// junctions) are counted as links, never followed: following them could
    /// count data outside the repository, twice, or loop forever.
    fn is_repository(&self, path: &Path) -> bool {
        path.join(".git").exists()
    }

    fn get_size(&self, path: &Path) -> std::io::Result<u64> {
        let meta = std::fs::symlink_metadata(path)?;
        if !meta.is_dir() {
            return Ok(meta.len());
        }
        let mut total = 0u64;
        let mut pending = vec![path.to_path_buf()];
        while let Some(dir) = pending.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                match entry.file_type() {
                    Ok(ft) if ft.is_dir() => pending.push(entry.path()),
                    Ok(_) => total += entry.metadata().map(|m| m.len()).unwrap_or(0),
                    Err(_) => {}
                }
            }
        }
        Ok(total)
    }
}

impl GitExecutor for RealSystem {
    fn run_git_command(&self, path: &Path, args: &[&str]) -> std::io::Result<String> {
        let output = git_command().args(args).current_dir(path).output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(std::io::Error::other(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }

    fn run_git_command_async(
        &self,
        path: &Path,
        args: &[&str],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = std::io::Result<String>> + Send + '_>>
    {
        let path = path.to_path_buf();
        let args: Vec<String> = args.iter().map(|&s| s.to_string()).collect();

        Box::pin(async move {
            let output = tokio::process::Command::from(git_command())
                .args(&args)
                .current_dir(&path)
                .output()
                .await?;

            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).to_string())
            } else {
                Err(std::io::Error::other(
                    String::from_utf8_lossy(&output.stderr).to_string(),
                ))
            }
        })
    }

    fn open_repo(&self, path: &Path) -> Result<(), crate::git::models::GitError> {
        gix::open(path)?;
        Ok(())
    }

    fn run_git_command_with_input(
        &self,
        path: &Path,
        args: &[&str],
        input: &str,
    ) -> std::io::Result<String> {
        use std::io::Write;
        use std::process::Stdio;

        let mut child = git_command()
            .args(args)
            .current_dir(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Feed stdin from another thread so a full stdout pipe cannot deadlock.
        let mut stdin = child.stdin.take().expect("stdin is piped");
        let input = input.to_owned();
        let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
        let output = child.wait_with_output()?;
        let _ = writer.join();

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(std::io::Error::other(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }
}

#[cfg(test)]
pub mod mock {
    use super::{FileSystem, GitExecutor};
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    #[derive(Clone)]
    pub struct MockSystem {
        pub file_sizes: HashMap<PathBuf, u64>,
        /// Directories reported as nested repositories.
        pub repositories: Vec<PathBuf>,
        pub command_outputs: HashMap<(PathBuf, Vec<String>), Result<String, String>>,
    }

    impl MockSystem {
        pub fn new() -> Self {
            Self {
                file_sizes: HashMap::new(),
                repositories: Vec::new(),
                command_outputs: HashMap::new(),
            }
        }

        pub fn add_command_output(
            &mut self,
            path: &Path,
            args: &[&str],
            output: Result<String, String>,
        ) {
            let args_vec = args.iter().map(|s| s.to_string()).collect();
            self.command_outputs
                .insert((path.to_path_buf(), args_vec), output);
        }

        /// Registers an output for a command that receives `input` on stdin.
        pub fn add_command_output_with_input(
            &mut self,
            path: &Path,
            args: &[&str],
            input: &str,
            output: Result<String, String>,
        ) {
            let mut key: Vec<&str> = args.to_vec();
            key.extend([STDIN_MARKER, input]);
            self.add_command_output(path, &key, output);
        }
    }

    const STDIN_MARKER: &str = "<stdin>";

    impl FileSystem for MockSystem {
        fn is_repository(&self, path: &Path) -> bool {
            self.repositories.iter().any(|r| r == path)
        }

        fn get_size(&self, path: &Path) -> std::io::Result<u64> {
            if let Some(&size) = self.file_sizes.get(path) {
                Ok(size)
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "File not found in mock",
                ))
            }
        }
    }

    impl GitExecutor for MockSystem {
        fn run_git_command(&self, path: &Path, args: &[&str]) -> std::io::Result<String> {
            let args_vec: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            if let Some(res) = self
                .command_outputs
                .get(&(path.to_path_buf(), args_vec.clone()))
            {
                match res {
                    Ok(s) => Ok(s.clone()),
                    Err(e) => Err(std::io::Error::other(e.clone())),
                }
            } else {
                println!("UNMOCKED COMMAND: path={:?}, args={:?}", path, args_vec);
                Ok(String::new()) // default to empty string if not mocked
            }
        }

        fn run_git_command_async(
            &self,
            path: &Path,
            args: &[&str],
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = std::io::Result<String>> + Send + '_>>
        {
            let res = self.run_git_command(path, args);
            Box::pin(async move { res })
        }

        fn open_repo(&self, _path: &Path) -> Result<(), crate::git::models::GitError> {
            Ok(())
        }

        fn run_git_command_with_input(
            &self,
            path: &Path,
            args: &[&str],
            input: &str,
        ) -> std::io::Result<String> {
            let mut key: Vec<&str> = args.to_vec();
            key.extend([STDIN_MARKER, input]);
            let key_vec: Vec<String> = key.iter().map(|s| s.to_string()).collect();
            if self
                .command_outputs
                .contains_key(&(path.to_path_buf(), key_vec))
            {
                self.run_git_command(path, &key)
            } else {
                self.run_git_command(path, args)
            }
        }
    }
}
