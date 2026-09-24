//! Command-line subcommands that run without the TUI.

use crate::journal::{self, EntryKind, Journal};

const LIST_LIMIT: usize = 30;

/// `sloth restore`: lists journal entries, or restores the given ones.
pub fn restore(ids: &[usize], last: bool) -> anyhow::Result<()> {
    let journal = Journal::open_default()
        .ok_or_else(|| anyhow::anyhow!("no data directory available for the journal"))?;
    let entries = journal.entries();
    if entries.is_empty() {
        println!("Nothing to restore: the journal is empty.");
        return Ok(());
    }

    // Ids are 1-based positions in the journal.
    let selected: Vec<usize> = if last {
        let batch = entries.last().map(|e| e.batch);
        (0..entries.len())
            .filter(|&i| Some(entries[i].batch) == batch)
            .collect()
    } else if ids.is_empty() {
        print_entries(&entries, journal.path());
        return Ok(());
    } else {
        ids.iter().map(|id| id.saturating_sub(1)).collect()
    };

    let sys = crate::sys::RealSystem;
    let mut failures = 0;
    for i in selected {
        let Some(entry) = entries.get(i) else {
            println!("❌ #{}: no such entry", i + 1);
            failures += 1;
            continue;
        };
        match journal::restore(entry, &sys) {
            Ok(msg) => println!("✅ #{} {}: {}", i + 1, entry.repo.display(), msg),
            Err(err) => {
                println!("❌ #{} {}: {}", i + 1, entry.repo.display(), err);
                failures += 1;
            }
        }
    }
    if failures > 0 {
        anyhow::bail!("{failures} entrie(s) could not be restored");
    }
    Ok(())
}

fn print_entries(entries: &[journal::Entry], path: &std::path::Path) {
    let now = crate::git::stats::now_ts();
    let skipped = entries.len().saturating_sub(LIST_LIMIT);
    println!("Recently deleted (journal: {}):\n", path.display());
    for (i, entry) in entries.iter().enumerate().skip(skipped) {
        let kind = match entry.kind {
            EntryKind::Branch => "branch",
            EntryKind::Stash => "stash ",
        };
        println!(
            "  #{:<4} {:>4} ago  {}  {:<30}  {}",
            i + 1,
            crate::git::stats::format_age(entry.ts, now),
            kind,
            entry.name,
            entry.repo.display()
        );
    }
    if skipped > 0 {
        println!("\n  ({skipped} older entries not shown)");
    }
    println!("\nRestore with `sloth restore <id>...`, or `sloth restore --last` for the last run.");
}
