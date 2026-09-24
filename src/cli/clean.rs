//! `sloth clean`: delete branches across repositories without the TUI, for
//! scripts and quick one-off cleanups.

use crate::cleanup::{self, branch_protection};
use crate::config::Config;
use crate::engine::{self, OpResult, RepoPlan};
use crate::git::RepoStatus;
use crate::git::stats::{format_age, format_size, now_ts};
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

#[derive(clap::Args, Debug, Default)]
pub struct CleanArgs {
    /// Delete branches merged into the default branch (including squash/rebase merges)
    #[arg(long)]
    pub merged: bool,
    /// Delete branches whose upstream was deleted on the remote
    #[arg(long)]
    pub gone: bool,
    /// Delete branches without a commit for DAYS days [default: `stale_days` from the config]
    #[arg(long, value_name = "DAYS", num_args = 0..=1, default_missing_value = "0")]
    pub stale: Option<u32>,
    /// Only consider branches whose name contains this text
    #[arg(long, value_name = "TEXT")]
    pub branch: Option<String>,
    /// Show what would be deleted, without deleting anything
    #[arg(short = 'n', long)]
    pub dry_run: bool,
    /// Do not ask for confirmation
    #[arg(short, long)]
    pub yes: bool,
}

impl CleanArgs {
    /// Without any criterion, clean what smart selection would: merged and gone.
    fn criteria(&self, config: &Config) -> (bool, bool, Option<u32>) {
        let stale = self
            .stale
            .map(|days| if days == 0 { config.stale_days } else { days });
        if !self.merged && !self.gone && stale.is_none() {
            (true, true, None)
        } else {
            (self.merged, self.gone, stale)
        }
    }
}

pub async fn clean(root: PathBuf, config: Config, args: CleanArgs) -> anyhow::Result<()> {
    let repos = analyze_all(&root, &config).await;
    let plans = plan(&repos, &config, &args);

    if plans.is_empty() {
        println!("Nothing to clean in {} repositories.", repos.len());
        return Ok(());
    }
    print_plan(&plans, &repos, &root);

    let count: usize = plans.iter().map(|p| p.operations.len()).sum();
    if args.dry_run {
        println!("\nDry run: {count} branch(es) would be deleted. Nothing was changed.");
        return Ok(());
    }
    if !args.yes && !confirm(count, plans.len())? {
        println!("Aborted, nothing was changed.");
        return Ok(());
    }

    let observer = crate::journal::Journal::open_default().map(|j| j.observer(now_ts()));
    let results = engine::execute(plans, false, crate::sys::RealSystem, observer).await;
    print_results(&results, &root);
    let failed = results.iter().filter(|r| !r.is_ok()).count();
    if failed > 0 {
        anyhow::bail!("{failed} operation(s) failed");
    }
    Ok(())
}

/// Discovers and analyzes every repository under `root`.
async fn analyze_all(root: &Path, config: &Config) -> Vec<RepoStatus> {
    let mut found = crate::scanner::scan_for_repositories(root, config.scan_exclude.clone());
    let slots = std::sync::Arc::new(tokio::sync::Semaphore::new(
        std::thread::available_parallelism().map_or(4, |n| n.get()),
    ));
    let mut analyses = tokio::task::JoinSet::new();
    while let Some(Ok(path)) = found.recv().await {
        let slots = slots.clone();
        analyses.spawn(async move {
            let _slot = slots.acquire_owned().await;
            tokio::task::spawn_blocking(move || {
                crate::git::analyze_repository(&path, &crate::sys::RealSystem)
            })
            .await
        });
    }

    let total = analyses.len();
    let interactive = std::io::stderr().is_terminal();
    let mut repos = Vec::with_capacity(total);
    while let Some(joined) = analyses.join_next().await {
        if let Ok(Ok(Ok(repo))) = joined {
            repos.push(repo);
        }
        if interactive {
            eprint!("\rAnalyzing repositories… {}/{}", repos.len(), total);
        }
    }
    if interactive {
        eprint!("\r\x1b[2K");
    }
    repos.sort_by(|a, b| crate::ui::views::alphabetical(&a.path, &b.path));
    repos
}

fn plan(repos: &[RepoStatus], config: &Config, args: &CleanArgs) -> Vec<RepoPlan> {
    let (merged, gone, stale_days) = args.criteria(config);
    let stale_config = stale_days.map(|days| Config {
        stale_days: days,
        ..config.clone()
    });
    let now = now_ts();
    repos
        .iter()
        .filter_map(|repo| {
            let names: Vec<&str> = repo
                .branches
                .iter()
                .filter(|b| branch_protection(repo, b, config).is_none())
                .filter(|b| {
                    args.branch
                        .as_deref()
                        .is_none_or(|text| b.name.contains(text))
                })
                .filter(|b| {
                    (merged && b.is_fully_merged())
                        || (gone && b.is_dead)
                        || stale_config
                            .as_ref()
                            .is_some_and(|c| cleanup::is_stale(b, c, now))
                })
                .map(|b| b.name.as_str())
                .collect();
            let operations = cleanup::cleanup_operations(repo, names, [], [], config);
            (!operations.is_empty()).then(|| RepoPlan {
                repo: repo.path.clone(),
                operations,
            })
        })
        .collect()
}

fn relative(path: &Path, root: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(rel) if !rel.as_os_str().is_empty() => rel.display().to_string(),
        _ => path.display().to_string(),
    }
}

fn print_plan(plans: &[RepoPlan], repos: &[RepoStatus], root: &Path) {
    let now = now_ts();
    for plan in plans {
        let Some(repo) = repos.iter().find(|r| r.path == plan.repo) else {
            continue;
        };
        println!("{}", relative(&repo.path, root));
        for op in &plan.operations {
            let engine::Operation::DeleteBranch { name, .. } = op else {
                continue;
            };
            let branch = repo.branches.iter().find(|b| &b.name == name);
            let mut tags = Vec::new();
            if let Some(b) = branch {
                if b.is_merged {
                    tags.push("merged".to_string());
                } else if b.is_squash_merged {
                    tags.push("squash-merged".to_string());
                }
                if b.is_dead {
                    tags.push("gone".to_string());
                }
                if let Some(ts) = b.last_commit_ts {
                    tags.push(format!("last commit {} ago", format_age(ts, now)));
                }
            }
            let warning = cleanup::operation_warning(repo, op)
                .map(|w| format!("  ! {w}"))
                .unwrap_or_default();
            println!("  - {name} ({}){warning}", tags.join(", "));
        }
    }
}

fn confirm(count: usize, repos: usize) -> anyhow::Result<bool> {
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("refusing to delete without confirmation: pass --yes, or --dry-run");
    }
    print!("\nDelete {count} branch(es) in {repos} repositorie(s)? [y/N] ");
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "Yes"))
}

fn print_results(results: &[OpResult], root: &Path) {
    let mut current: Option<&Path> = None;
    for res in results {
        if current != Some(res.repo.as_path()) {
            println!("\n{}", relative(&res.repo, root));
            current = Some(res.repo.as_path());
        }
        match &res.outcome {
            Ok(msg) => println!("  ✓ {}: {}", res.operation.describe(), msg),
            Err(err) => println!("  ✗ {}: {}", res.operation.describe(), err),
        }
    }
    let freed: u64 = results.iter().map(|r| r.freed_bytes).sum();
    let failed = results.iter().filter(|r| !r.is_ok()).count();
    println!(
        "\n{} succeeded, {} failed, {} freed. Undo with `sloth restore --last`.",
        results.len() - failed,
        failed,
        format_size(freed)
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::models::BranchInfo;

    fn repo() -> RepoStatus {
        let mut repo = RepoStatus::pending(PathBuf::from("/r"));
        repo.default_branch = Some("main".into());
        let day = 86_400;
        repo.branches = vec![
            BranchInfo {
                name: "main".into(),
                is_merged: true,
                ..Default::default()
            },
            BranchInfo {
                name: "merged".into(),
                is_merged: true,
                last_commit_ts: Some(now_ts()),
                ..Default::default()
            },
            BranchInfo {
                name: "gone".into(),
                is_dead: true,
                ahead: 1,
                last_commit_ts: Some(now_ts()),
                ..Default::default()
            },
            BranchInfo {
                name: "old-wip".into(),
                ahead: 4,
                last_commit_ts: Some(now_ts() - 200 * day),
                ..Default::default()
            },
        ];
        repo
    }

    fn deleted(args: CleanArgs) -> Vec<String> {
        plan(&[repo()], &Config::default(), &args)
            .into_iter()
            .flat_map(|p| p.operations)
            .map(|op| match op {
                engine::Operation::DeleteBranch { name, .. } => name,
                other => other.describe(),
            })
            .collect()
    }

    #[test]
    fn defaults_to_merged_and_gone() {
        assert_eq!(deleted(CleanArgs::default()), vec!["gone", "merged"]);
    }

    #[test]
    fn criteria_combine_and_skip_protected() {
        let args = CleanArgs {
            merged: true,
            ..Default::default()
        };
        assert_eq!(deleted(args), vec!["merged"]);

        let args = CleanArgs {
            stale: Some(0), // `--stale` without a value: the config's 90 days
            ..Default::default()
        };
        assert_eq!(deleted(args), vec!["old-wip"]);

        let args = CleanArgs {
            stale: Some(365),
            ..Default::default()
        };
        assert!(deleted(args).is_empty());
    }

    #[test]
    fn branch_filter_narrows_the_selection() {
        let args = CleanArgs {
            branch: Some("gon".into()),
            ..Default::default()
        };
        assert_eq!(deleted(args), vec!["gone"]);
    }
}
