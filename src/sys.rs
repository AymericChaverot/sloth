use std::path::Path;

pub trait FileSystem {
    fn exists(&self, path: &Path) -> bool;
    fn get_size(&self, path: &Path) -> std::io::Result<u64>;
}

pub trait GitExecutor {
    fn run_git_command(&self, path: &Path, args: &[&str]) -> std::io::Result<String>;
    fn run_git_command_async(
        &self,
        path: &Path,
        args: &[&str],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = std::io::Result<String>> + Send + '_>>;
    fn open_repo(&self, path: &Path) -> Result<(), crate::git::models::GitError>;
}

#[derive(Clone)]
pub struct RealSystem;

impl FileSystem for RealSystem {
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn get_size(&self, path: &Path) -> std::io::Result<u64> {
        if path.is_dir() {
            let mut total = 0u64;
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                let entry_path = entry.path();
                if entry_path.is_dir() {
                    total += self.get_size(&entry_path).unwrap_or(0);
                } else {
                    total += entry.metadata().map(|m| m.len()).unwrap_or(0);
                }
            }
            Ok(total)
        } else {
            Ok(std::fs::metadata(path)?.len())
        }
    }
}

impl GitExecutor for RealSystem {
    fn run_git_command(&self, path: &Path, args: &[&str]) -> std::io::Result<String> {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(path)
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
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
            let output = tokio::process::Command::new("git")
                .args(&args)
                .current_dir(&path)
                .output()
                .await?;

            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).to_string())
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    String::from_utf8_lossy(&output.stderr).to_string(),
                ))
            }
        })
    }

    fn open_repo(&self, path: &Path) -> Result<(), crate::git::models::GitError> {
        gix::open(path)?;
        Ok(())
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
        pub directories: Vec<PathBuf>,
        pub files: Vec<PathBuf>,
        pub command_outputs: HashMap<(PathBuf, Vec<String>), Result<String, String>>,
    }

    impl MockSystem {
        pub fn new() -> Self {
            Self {
                file_sizes: HashMap::new(),
                directories: Vec::new(),
                files: Vec::new(),
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
    }

    impl FileSystem for MockSystem {
        fn exists(&self, path: &Path) -> bool {
            self.files.contains(&path.to_path_buf())
                || self.directories.contains(&path.to_path_buf())
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
                    Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, e.clone())),
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
    }
}
