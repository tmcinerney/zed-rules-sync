mod db;
mod types;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use db::{default_db_path, is_zed_running, RulesDb};
use std::collections::HashSet;
use std::path::PathBuf;
use types::*;

#[derive(Parser)]
#[command(
    name = "zed-rules-sync",
    about = "Sync markdown rule files into Zed's Rules Library",
    version
)]
struct Cli {
    #[arg(long, global = true)]
    db_path: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List all rules in Zed's Rules Library
    List,
    /// Sync markdown files into the Rules Library
    Sync {
        /// Directory of .md files or a single .md file
        path: PathBuf,
        /// Mark synced rules as default (auto-included in every thread)
        #[arg(long)]
        default: bool,
        /// Remove managed rules whose source .md no longer exists
        #[arg(long)]
        prune: bool,
        /// Show what would happen without making changes
        #[arg(long)]
        dry_run: bool,
        /// Recursively walk subdirectories for .md files
        #[arg(long, short = 'r')]
        recursive: bool,
    },
    /// Remove rules from the Rules Library
    Remove {
        /// Title or UUID of a specific rule to remove
        title_or_uuid: Option<String>,
        /// Remove all rules created by this tool
        #[arg(long)]
        managed: bool,
        /// Show what would happen without making changes
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let db_path = cli.db_path.unwrap_or_else(default_db_path);
    match cli.command {
        Commands::List => cmd_list(&db_path),
        Commands::Sync {
            path,
            default,
            prune,
            dry_run,
            recursive,
        } => cmd_sync(&db_path, &path, default, prune, dry_run, recursive),
        Commands::Remove {
            title_or_uuid,
            managed,
            dry_run,
        } => cmd_remove(&db_path, title_or_uuid, managed, dry_run),
    }
}

fn cmd_list(db_path: &PathBuf) -> Result<()> {
    let db = RulesDb::open_readonly(db_path)?;
    let entries = db.list_rules()?;
    if entries.is_empty() {
        println!("No rules found.");
        return Ok(());
    }
    println!("{:<40} {:<8} {:<8} UUID", "TITLE", "DEFAULT", "MANAGED");
    println!("{}", "-".repeat(90));
    for e in &entries {
        let title = e.metadata.title.as_deref().unwrap_or("Untitled");
        let managed = if is_managed(&e.id, e.metadata.title.as_deref()) {
            "yes"
        } else {
            ""
        };
        let def = if e.metadata.default { "yes" } else { "" };
        let uuid = match &e.id {
            PromptId::User { uuid } => uuid.0.to_string(),
            PromptId::BuiltIn(b) => format!("{:?}", b),
        };
        println!("{:<40} {:<8} {:<8} {}", title, def, managed, uuid);
    }
    println!("\n{} rule(s) total.", entries.len());
    Ok(())
}

fn cmd_sync(
    db_path: &PathBuf,
    path: &PathBuf,
    default: bool,
    prune: bool,
    dry_run: bool,
    recursive: bool,
) -> Result<()> {
    let md_files = collect_md_files(path, recursive)?;
    if md_files.is_empty() {
        println!("No .md files found at {}", path.display());
        return Ok(());
    }
    if !dry_run && is_zed_running() {
        eprintln!("Warning: Zed is running. Changes won't be visible until restart.");
    }
    let db = if dry_run {
        if db_path.exists() {
            Some(RulesDb::open_readonly(db_path)?)
        } else {
            None
        }
    } else {
        Some(RulesDb::open(db_path)?)
    };
    let mut created = 0u32;
    let mut updated = 0u32;
    let mut synced: HashSet<String> = HashSet::new();
    for (filename, filepath) in &md_files {
        let id = prompt_id_for_filename(filename);
        let title = title_from_filename(filename);
        let body = std::fs::read_to_string(filepath)
            .with_context(|| format!("failed to read {}", filepath.display()))?;
        synced.insert(filename.clone());
        let exists = db
            .as_ref()
            .map(|d| d.has_rule(&id).unwrap_or(false))
            .unwrap_or(false);
        if dry_run {
            println!(
                "  {}: {} ({})",
                if exists { "update" } else { "create" },
                title,
                filename
            );
        } else if let Some(ref db) = db {
            db.upsert_rule(id, &title, default, &body)?;
        }
        if exists {
            updated += 1;
        } else {
            created += 1;
        }
    }
    let mut pruned = 0u32;
    if prune {
        if let Some(ref db) = db {
            for entry in db.list_rules()? {
                if is_managed(&entry.id, entry.metadata.title.as_deref()) {
                    let t = entry.metadata.title.as_deref().unwrap_or("");
                    let fname = title_to_filename(t);
                    if !synced.contains(&fname) {
                        if dry_run {
                            println!("  prune: {} (source removed)", t);
                        } else {
                            db.delete_rule(entry.id)?;
                        }
                        pruned += 1;
                    }
                }
            }
        }
    }
    let p = if dry_run { "Would sync" } else { "Synced" };
    println!(
        "\n{}: {} created, {} updated, {} pruned",
        p, created, updated, pruned
    );
    Ok(())
}

fn cmd_remove(
    db_path: &PathBuf,
    title_or_uuid: Option<String>,
    managed: bool,
    dry_run: bool,
) -> Result<()> {
    if !managed && title_or_uuid.is_none() {
        anyhow::bail!("Specify a rule to remove or use --managed");
    }
    if !dry_run && is_zed_running() {
        eprintln!("Warning: Zed is running. Changes won't be visible until restart.");
    }
    let db = if dry_run {
        RulesDb::open_readonly(db_path)?
    } else {
        RulesDb::open(db_path)?
    };
    let mut removed = 0u32;
    if managed {
        for entry in db.list_rules()? {
            if is_managed(&entry.id, entry.metadata.title.as_deref()) {
                let t = entry.metadata.title.as_deref().unwrap_or("Untitled");
                if dry_run {
                    println!("  would remove: {}", t);
                } else {
                    db.delete_rule(entry.id)?;
                    println!("  removed: {}", t);
                }
                removed += 1;
            }
        }
    } else if let Some(ref needle) = title_or_uuid {
        let entries = db.list_rules()?;
        let target = entries.iter().find(|e| {
            e.metadata
                .title
                .as_deref()
                .map(|t| t.eq_ignore_ascii_case(needle))
                .unwrap_or(false)
                || matches!(&e.id, PromptId::User { uuid } if uuid.0.to_string() == *needle)
        });
        if let Some(entry) = target {
            let t = entry.metadata.title.as_deref().unwrap_or("Untitled");
            if dry_run {
                println!("  would remove: {}", t);
            } else {
                db.delete_rule(entry.id)?;
                println!("  removed: {}", t);
            }
            removed += 1;
        } else {
            println!("No rule found matching \"{}\"", needle);
        }
    }
    let p = if dry_run { "Would remove" } else { "Removed" };
    println!("\n{} {} rule(s).", p, removed);
    Ok(())
}

fn collect_md_files(path: &PathBuf, recursive: bool) -> Result<Vec<(String, PathBuf)>> {
    if path.is_file() {
        let name = path
            .file_name()
            .context("invalid path")?
            .to_string_lossy()
            .to_string();
        if name.ends_with(".md") {
            return Ok(vec![(name, path.clone())]);
        } else {
            anyhow::bail!("not a .md file: {}", path.display());
        }
    }
    if !path.is_dir() {
        anyhow::bail!("not a file or directory: {}", path.display());
    }
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    if recursive {
        walk_md_files(path, &mut files)?;
        check_duplicate_basenames(&files)?;
    } else {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".md") && entry.file_type()?.is_file() {
                files.push((name, entry.path()));
            }
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    Ok(files)
}

fn walk_md_files(dir: &PathBuf, out: &mut Vec<(String, PathBuf)>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            walk_md_files(&path, out)?;
        } else if file_type.is_file() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".md") {
                out.push((name, path));
            }
        }
    }
    Ok(())
}

fn check_duplicate_basenames(files: &[(String, PathBuf)]) -> Result<()> {
    use std::collections::BTreeMap;
    let mut groups: BTreeMap<&str, Vec<&PathBuf>> = BTreeMap::new();
    for (name, path) in files {
        groups.entry(name.as_str()).or_default().push(path);
    }
    for (name, paths) in groups {
        if paths.len() > 1 {
            let mut sorted: Vec<&PathBuf> = paths;
            sorted.sort();
            let mut msg = format!("duplicate filename \"{}\" at:", name);
            for p in sorted {
                msg.push_str(&format!("\n  {}", p.display()));
            }
            anyhow::bail!(msg);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn recursive_collects_nested_files() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::write(root.join("top.md"), "top").unwrap();
        let sub = root.join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("nested.md"), "nested").unwrap();
        let deeper = sub.join("deeper");
        fs::create_dir(&deeper).unwrap();
        fs::write(deeper.join("deep.md"), "deep").unwrap();
        fs::write(root.join("ignored.txt"), "ignored").unwrap();

        let files = collect_md_files(&root.to_path_buf(), true).unwrap();
        let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["deep.md", "nested.md", "top.md"]);
    }

    #[test]
    fn non_recursive_skips_subdirectories() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::write(root.join("top.md"), "top").unwrap();
        let sub = root.join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("nested.md"), "nested").unwrap();

        let files = collect_md_files(&root.to_path_buf(), false).unwrap();
        let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["top.md"]);
    }

    #[test]
    fn recursive_errors_on_duplicate_basename() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let a = root.join("a");
        let b = root.join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        let a_path = a.join("foo.md");
        let b_path = b.join("foo.md");
        fs::write(&a_path, "a").unwrap();
        fs::write(&b_path, "b").unwrap();

        let err =
            collect_md_files(&root.to_path_buf(), true).expect_err("expected duplicate error");
        let msg = format!("{}", err);
        assert!(
            msg.contains("duplicate filename \"foo.md\""),
            "msg: {}",
            msg
        );
        assert!(msg.contains(&a_path.display().to_string()), "msg: {}", msg);
        assert!(msg.contains(&b_path.display().to_string()), "msg: {}", msg);
        let a_idx = msg.find(&a_path.display().to_string()).unwrap();
        let b_idx = msg.find(&b_path.display().to_string()).unwrap();
        assert!(a_idx < b_idx, "paths should be sorted: {}", msg);
    }
}
