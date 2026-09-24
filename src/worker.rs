//! Background pipeline: discovery → analysis → disk usage, streamed to the UI.
//!
//! Each repository is analyzed as soon as it is discovered (bounded
//! concurrency), and its disk usage is queued right after, measured by a
//! few threads so a huge repository does not hold back the others.

use crate::git::RepoStatus;
use crate::sys::{FileSystem as _, RealSystem};
use crate::ui::ScannerEvent;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::Sender;

/// Repositories measured at the same time: enough to overlap slow ones,
/// few enough not to thrash the disk.
const SIZE_THREADS: usize = 4;

struct SizeJob {
    repo: PathBuf,
    linked_worktrees: Vec<String>,
}

#[derive(Clone)]
pub struct Worker {
    events: Sender<ScannerEvent>,
    sizes: Sender<SizeJob>,
    analysis_slots: Arc<tokio::sync::Semaphore>,
}

impl Worker {
    /// `deep_clean_keep`: patterns excluded from the reclaimable size.
    pub fn new(events: Sender<ScannerEvent>, deep_clean_keep: Vec<String>) -> Self {
        let (sizes, jobs) = std::sync::mpsc::channel::<SizeJob>();
        let jobs = Arc::new(std::sync::Mutex::new(jobs));
        let keep = Arc::new(deep_clean_keep);
        for _ in 0..SIZE_THREADS {
            let (jobs, keep, events) = (jobs.clone(), keep.clone(), events.clone());
            std::thread::spawn(move || {
                loop {
                    let job = match jobs.lock() {
                        Ok(jobs) => jobs.recv(),
                        Err(_) => return,
                    };
                    match job {
                        Ok(job) => measure(&job, &keep, &events),
                        Err(_) => return,
                    }
                }
            });
        }
        let parallelism = std::thread::available_parallelism().map_or(4, |n| n.get());
        Self {
            events,
            sizes,
            analysis_slots: Arc::new(tokio::sync::Semaphore::new(parallelism)),
        }
    }

    /// Discovers repositories under `root` and analyzes each one as it is found.
    pub fn scan(&self, root: PathBuf, exclude: Vec<String>) {
        let worker = self.clone();
        tokio::spawn(async move {
            let mut found = crate::scanner::scan_for_repositories(&root, exclude);
            let mut analyses = tokio::task::JoinSet::new();
            while let Some(res) = found.recv().await {
                if let Ok(path) = res {
                    let _ = worker.events.send(ScannerEvent::RepoFound(path.clone()));
                    analyses.spawn(worker.clone().analyze_one(path));
                }
            }
            let _ = worker.events.send(ScannerEvent::ScanComplete);
            while analyses.join_next().await.is_some() {}
            let _ = worker.events.send(ScannerEvent::AnalysisComplete);
        });
    }

    /// Re-analyzes (and re-measures) the given repositories, e.g. after a cleanup.
    pub fn refresh(&self, paths: Vec<PathBuf>) {
        let worker = self.clone();
        tokio::spawn(async move {
            let mut analyses = tokio::task::JoinSet::new();
            for path in paths {
                analyses.spawn(worker.clone().analyze_one(path));
            }
            while analyses.join_next().await.is_some() {}
            let _ = worker.events.send(ScannerEvent::AnalysisComplete);
        });
    }

    async fn analyze_one(self, path: PathBuf) {
        let Ok(_slot) = self.analysis_slots.clone().acquire_owned().await else {
            return;
        };
        let target = path.clone();
        let analysis = tokio::task::spawn_blocking(move || {
            crate::git::analyze_repository(&target, &RealSystem)
        })
        .await;
        match analysis {
            Ok(Ok(status)) => {
                let _ = self.sizes.send(SizeJob {
                    repo: status.path.clone(),
                    linked_worktrees: linked_worktrees(&status),
                });
                let _ = self.events.send(ScannerEvent::RepoAnalyzed(status));
            }
            Ok(Err(e)) => {
                let _ = self.events.send(ScannerEvent::RepoFailed {
                    path,
                    error: e.to_string(),
                });
            }
            Err(e) => {
                let _ = self.events.send(ScannerEvent::RepoFailed {
                    path,
                    error: e.to_string(),
                });
            }
        }
    }
}

fn linked_worktrees(status: &RepoStatus) -> Vec<String> {
    status
        .worktrees
        .iter()
        .filter(|w| !w.is_main && !w.is_prunable)
        .map(|w| w.path.clone())
        .collect()
}

/// Streams `.git` and untracked sizes as they are measured, then worktree sizes.
fn measure(job: &SizeJob, keep: &[String], events: &Sender<ScannerEvent>) {
    let sys = RealSystem;
    let repo = job.repo.as_path();
    let git_size = sys.get_size(&repo.join(".git")).ok();
    let _ = events.send(ScannerEvent::SizePartial {
        path: job.repo.clone(),
        size_bytes: git_size,
        untracked_size_bytes: 0,
    });

    let untracked = crate::engine::deep_clean_candidates(repo, keep, &sys)
        .ok()
        .map(|paths| {
            let mut total = 0u64;
            for path in paths {
                total += sys.get_size(&repo.join(&path)).unwrap_or(0);
                let _ = events.send(ScannerEvent::SizePartial {
                    path: job.repo.clone(),
                    size_bytes: git_size,
                    untracked_size_bytes: total,
                });
            }
            total
        });

    let worktree_sizes: HashMap<String, Option<u64>> = job
        .linked_worktrees
        .iter()
        .map(|wt| (wt.clone(), sys.get_size(Path::new(wt)).ok()))
        .collect();

    let _ = events.send(ScannerEvent::SizeComputed {
        path: job.repo.clone(),
        size_bytes: git_size,
        untracked_size_bytes: untracked,
        worktree_sizes,
    });
}

#[cfg(test)]
mod bench {
    /// `SLOTH_BENCH_DIR=... cargo test --release worker::bench -- --ignored --nocapture`
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn bench_pipeline() {
        use crate::ui::ScannerEvent;
        let Some(dir) = std::env::var_os("SLOTH_BENCH_DIR") else {
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = super::Worker::new(tx, crate::config::Config::default().deep_clean_keep);
        let start = std::time::Instant::now();
        worker.scan(dir.into(), crate::config::Config::default().scan_exclude);
        let (mut found, mut sized, mut analysis_done) = (0, 0, false);
        loop {
            match rx.recv_timeout(std::time::Duration::from_secs(120)) {
                Ok(ScannerEvent::RepoFound(_)) => found += 1,
                Ok(ScannerEvent::ScanComplete) => {
                    println!("scan complete: {:?} ({found} repos)", start.elapsed())
                }
                Ok(ScannerEvent::AnalysisComplete) => {
                    analysis_done = true;
                    println!("analysis complete: {:?}", start.elapsed());
                }
                Ok(ScannerEvent::SizeComputed { .. }) => sized += 1,
                Ok(_) => {}
                Err(_) => break,
            }
            if analysis_done && sized == found {
                println!("sizes complete: {:?}", start.elapsed());
                break;
            }
        }
    }
}
