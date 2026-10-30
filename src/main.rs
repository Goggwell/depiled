use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use depiled::model::{Collection, Game};
use depiled::ops::Op;
use depiled::steam::SteamInstall;
use depiled::{bundle, cdp, config, live, rules, service, snapshot, steam};

#[derive(Parser)]
#[command(about = "Manage Steam library collections with rules, backups and undo")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List collections read from Steam's files on disk (read-only, works any time)
    List,
    /// Show what the rules would change. Changes nothing.
    Plan {
        #[arg(long, default_value_t = cdp::DEFAULT_PORT)]
        port: u16,
    },
    /// Apply the rules: snapshot first, then change collections, then verify
    Apply {
        #[arg(long, default_value_t = cdp::DEFAULT_PORT)]
        port: u16,
    },
    /// Create or inspect the rules file
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Save a snapshot of all collections and Steam's cloudstorage files
    Snapshot {
        #[arg(default_value = "manual")]
        label: String,
    },
    /// Show saved snapshots, newest first
    Snapshots,
    /// Restore collections to a snapshot (newest by default). Shows the plan unless --apply.
    Undo {
        /// Snapshot folder name or timestamp prefix, see `snapshots`
        snapshot: Option<String>,
        #[arg(long)]
        apply: bool,
        #[arg(long, default_value_t = cdp::DEFAULT_PORT)]
        port: u16,
    },
    /// Write a portable backup (collections + rules) to a file
    Export {
        file: PathBuf,
        #[arg(long, default_value_t = cdp::DEFAULT_PORT)]
        port: u16,
    },
    /// Restore collections from a backup file, e.g. on another device. Shows the plan unless --apply.
    Import {
        file: PathBuf,
        #[arg(long)]
        apply: bool,
        /// Also replace the rules file with the one in the backup (the old one is kept)
        #[arg(long)]
        with_config: bool,
        #[arg(long, default_value_t = cdp::DEFAULT_PORT)]
        port: u16,
    },
    /// Smart collections: create one that fills itself, or top one up
    Smart {
        #[command(subcommand)]
        action: SmartAction,
        #[arg(long, default_value_t = cdp::DEFAULT_PORT, global = true)]
        port: u16,
    },
    /// Show or set how automatic organizing is: off, notify or auto
    Automation { mode: Option<String> },
    /// Watch Steam and report when the rules have something to do (Ctrl+C to stop)
    Watch {
        #[arg(long, default_value_t = cdp::DEFAULT_PORT)]
        port: u16,
    },
    /// Summarize what the desktop app sees: games, cover art, tags (read-only)
    Library {
        #[arg(long, default_value_t = cdp::DEFAULT_PORT)]
        port: u16,
    },
    /// Read-only check of the running Steam client's collection store over the debug port
    Probe {
        #[arg(long, default_value_t = cdp::DEFAULT_PORT)]
        port: u16,
    },
    /// Developer tool: run raw JavaScript in Steam's library context. Can modify Steam.
    Eval {
        expression: String,
        #[arg(long, default_value_t = cdp::DEFAULT_PORT)]
        port: u16,
    },
}

#[derive(clap::Args, Clone)]
struct Criteria {
    /// never, played or idle
    #[arg(long)]
    played: Option<String>,
    /// complete or incomplete
    #[arg(long)]
    achievements: Option<String>,
    #[arg(long, default_value_t = 0)]
    min_review: u32,
    /// Genre or tag name, e.g. Horror (repeat for several)
    #[arg(long = "tag")]
    tags: Vec<String>,
    /// Steam flag name or id, e.g. "Single-player" (repeat for several)
    #[arg(long = "flag")]
    flags: Vec<String>,
}

#[derive(Subcommand)]
enum SmartAction {
    /// How many games the criteria match right now. Changes nothing.
    Preview {
        #[command(flatten)]
        criteria: Criteria,
    },
    /// Create a collection filled with matching games
    Create {
        name: String,
        #[command(flatten)]
        criteria: Criteria,
        /// Remember the criteria so `fill` can top the collection up later
        #[arg(long)]
        auto: bool,
    },
    /// Add games matching a collection's saved criteria
    Fill { id: String },
    /// Show saved criteria by collection id
    List,
}

fn smart_rule(c: &Criteria, live: &live::Live) -> Result<depiled::smart::SmartRule> {
    let names = live.tag_names()?;
    let tag_id = |t: &String| {
        t.parse::<u32>().ok().or_else(|| names.iter().find(|(_, n)| n.eq_ignore_ascii_case(t)).map(|(id, _)| *id))
            .with_context(|| format!("Steam has no tag or genre called '{t}'"))
    };
    Ok(depiled::smart::SmartRule {
        played: c.played.clone(),
        achievements: c.achievements.clone(),
        min_review: c.min_review,
        tags: c.tags.iter().map(tag_id).collect::<Result<_>>()?,
        flags: c
            .flags
            .iter()
            .map(|f| rules::flag_id(f).with_context(|| format!("unknown flag '{f}'")))
            .collect::<Result<_>>()?,
    })
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Write a starter config.toml (never overwrites an existing one)
    Init,
    /// Print the config file location
    Path,
    /// Validate and summarize the rules
    Check,
}

const PROBE_JS: &str = r#"(() => {
  const cs = window.collectionStore;
  if (!cs) return { error: "no collectionStore on window" };
  return { count: cs.userCollections ? cs.userCollections.length : null,
           names: (cs.userCollections || []).map(c => c.displayName || c.name || c.id) };
})()"#;

fn make_plan(port: u16) -> Result<service::PlanData> {
    let plan = service::build_plan(port)?;
    for note in &plan.notes {
        println!("Note: {note}");
    }
    Ok(plan)
}

fn print_ops(ops: &[Op], games: &[Game]) {
    let names: HashMap<u32, &str> = games.iter().map(|g| (g.appid, g.name.as_str())).collect();
    for op in ops {
        println!("  {op}");
        let apps = match op {
            Op::Create { apps, .. } | Op::Add { apps, .. } | Op::Remove { apps, .. } => apps,
            Op::Rename { .. } | Op::Delete { .. } => continue,
        };
        let sample: Vec<&str> = apps.iter().take(5).map(|a| names.get(a).copied().unwrap_or("?")).collect();
        let more = apps.len().saturating_sub(sample.len());
        let tail = if more > 0 { format!(", and {more} more") } else { String::new() };
        println!("      {}{tail}", sample.join(", "));
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::List => {
            let install = SteamInstall::detect()?;
            for c in steam::read_collections_from_disk(&install.cloudstorage_dir())? {
                let kind = if c.dynamic { " [dynamic]" } else { "" };
                println!("{:<18} {:<30} {} games{kind}", c.id, c.name, c.apps.len());
            }
        }
        Command::Plan { port } => {
            let plan = make_plan(port)?;
            println!("{} games read, {} collections.", plan.games.len(), plan.current.len());
            if plan.ops.is_empty() {
                println!("Nothing to change.");
            } else {
                print_ops(&plan.ops, &plan.games);
                println!("Dry run. Run `apply` to make these changes (a snapshot is taken first).");
            }
        }
        Command::Apply { port } => {
            let plan = make_plan(port)?;
            if plan.ops.is_empty() {
                println!("Nothing to change.");
                return Ok(());
            }
            print_ops(&plan.ops, &plan.games);
            let install = SteamInstall::detect()?;
            let folder = snapshot::save(
                &snapshot::store_dir()?,
                "before-apply",
                plan.current.clone(),
                &install.cloudstorage_dir(),
            )?;
            println!("Snapshot saved: {} (undo with `undo`)", folder.display());
            for op in &plan.ops {
                plan.live.apply(op)?;
                println!("  done: {op}");
            }
            let after = plan.live.collections()?;
            let leftover = rules::plan(&plan.rules, &plan.games, &after, snapshot::now_secs());
            if leftover.is_empty() {
                println!("Verified: collections now follow the rules.");
            } else {
                println!("Warning: {} change(s) did not apply:", leftover.len());
                print_ops(&leftover, &plan.games);
            }
            if let Some(dir) = &plan.config.backup_dir {
                let path = bundle::auto_backup(dir, after, Some(plan.config_text), 10)?;
                println!("Backup written to {}", path.display());
            }
        }
        Command::Config { action } => match action {
            ConfigAction::Init => println!("Created {}", config::init()?.display()),
            ConfigAction::Path => println!("{}", config::path()?.display()),
            ConfigAction::Check => {
                let (cfg, _) = config::load()?;
                for r in &cfg.rules {
                    let group = r.group.as_deref().unwrap_or("-");
                    println!("{:<20} group={group:<8} sticky={} sync={}", r.collection, r.sticky, r.sync);
                }
                println!("{} rules OK.", cfg.rules.len());
            }
        },
        Command::Snapshot { label } => {
            let install = SteamInstall::detect()?;
            let cols = steam::read_collections_from_disk(&install.cloudstorage_dir())?;
            let folder = snapshot::save(&snapshot::store_dir()?, &label, cols, &install.cloudstorage_dir())?;
            println!("Saved snapshot to {}", folder.display());
        }
        Command::Snapshots => {
            for (path, snap) in snapshot::list(&snapshot::store_dir()?)? {
                println!("{}  {:<20} {} collections", snap.taken_at, snap.label, snap.collections.len());
                println!("  {}", path.display());
            }
        }
        Command::Undo { snapshot: wanted, apply, port } => {
            let dir = snapshot::store_dir()?;
            let snaps = snapshot::list(&dir)?;
            let (path, snap) = match &wanted {
                Some(w) => snaps
                    .into_iter()
                    .find(|(p, _)| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(w.as_str())))
                    .with_context(|| format!("no snapshot matching '{w}'"))?,
                None => snaps.into_iter().next().context("no snapshots saved yet")?,
            };
            println!("Snapshot: {}", path.display());
            restore(&snap.collections, apply, port, "before-undo")?;
        }
        Command::Export { file, port } => {
            let live = live::Live::connect(port)?;
            let config_text = config::load().ok().map(|(_, t)| t);
            bundle::write(&file, live.collections()?, config_text)?;
            println!("Wrote {}", file.display());
        }
        Command::Import { file, apply, with_config, port } => {
            let bundle = bundle::read(&file)?;
            restore(&bundle.collections, apply, port, "before-import")?;
            if with_config {
                match (&bundle.config, apply) {
                    (Some(text), true) => {
                        config::replace_with_backup(text)?;
                        println!("Rules file replaced (previous one kept in config-backups).");
                    }
                    (Some(_), false) => println!("Would replace the rules file with the backup's."),
                    (None, _) => println!("The backup contains no rules file."),
                }
            }
        }
        Command::Eval { expression, port } => {
            let ws = cdp::shared_context_url(port)?;
            println!("{}", serde_json::to_string_pretty(&cdp::eval(&ws, &expression)?)?);
        }
        Command::Smart { action, port } => match action {
            SmartAction::Preview { criteria } => {
                let rule = smart_rule(&criteria, &live::Live::connect(port)?)?;
                let (total, _) = service::preview_smart(port, &rule, None)?;
                println!("{total} games match.");
            }
            SmartAction::Create { name, criteria, auto } => {
                let rule = smart_rule(&criteria, &live::Live::connect(port)?)?;
                let r = service::create_smart_collection(port, &name, &rule, auto)?;
                println!("Created '{name}' with {} games. Snapshot: {}", r.added, r.snapshot.unwrap_or_default());
            }
            SmartAction::Fill { id } => {
                let r = service::fill_collection(port, &id)?;
                println!("Added {} games. Snapshot: {}", r.added, r.snapshot.unwrap_or_else(|| "none needed".into()));
            }
            SmartAction::List => {
                for (id, rule) in service::smart_rules(port)? {
                    println!("{id:<20} {}", serde_json::to_string(&rule)?);
                }
            }
        },
        Command::Automation { mode } => {
            let mut settings = service::get_settings()?;
            if let Some(mode) = mode {
                settings.automation = Some(match mode.as_str() {
                    "off" => depiled::settings::Automation::Off,
                    "notify" => depiled::settings::Automation::Notify,
                    "auto" => depiled::settings::Automation::Auto,
                    other => anyhow::bail!("unknown mode '{other}'; use off, notify or auto"),
                });
                service::set_settings(&settings)?;
            }
            match settings.automation {
                None => println!("Not chosen yet (the app will ask once)."),
                Some(m) => println!("{m:?}"),
            }
        }
        Command::Watch { port } => {
            let stop = std::sync::atomic::AtomicBool::new(false);
            println!("Watching Steam. Change the mode with `automation off|notify|auto`.");
            depiled::watcher::run(port, &stop, &mut |event| match event {
                depiled::watcher::Event::Pending { ops, .. } => {
                    println!("Ready to organize:");
                    for op in ops {
                        println!("  {} {} ({} games)", op.kind, op.collection, op.count);
                    }
                }
                depiled::watcher::Event::Organized { applied, verified, .. } => {
                    println!("Organized automatically: {applied} changes, verified: {verified}");
                }
            });
        }
        Command::Library { port } => {
            let lib = service::load_library(port)?;
            let with_cover = lib.games.iter().filter(|g| g.cover.is_some()).count();
            let uncategorized = lib.games.iter().filter(|g| g.collections.is_empty()).count();
            println!("{} games, {} with cover art, {} in no collection", lib.games.len(), with_cover, uncategorized);
            println!("{} tags known, {} flags", lib.tags.len(), lib.flags.len());
            for c in &lib.collections {
                println!("  {:<28} {:>4}{}", c.name, c.count, if c.dynamic { "  (Steam-managed)" } else { "" });
            }
            if let Some(g) = lib.games.iter().find(|g| g.cover.is_some()) {
                println!("sample: {} -> {}", g.name, g.cover.as_deref().unwrap_or(""));
            }
        }
        Command::Probe { port } => {
            let ws = cdp::shared_context_url(port)?;
            println!("{}", serde_json::to_string_pretty(&cdp::eval(&ws, PROBE_JS)?)?);
        }
    }
    Ok(())
}

/// Shared by `undo` and `import`: diff the live state against `target`, optionally apply.
fn restore(target: &[Collection], apply: bool, port: u16, safety_label: &str) -> Result<()> {
    let report = service::restore(port, target, apply, false, safety_label)?;
    if report.changes.is_empty() {
        println!("Nothing to change; collections already match.");
        return Ok(());
    }
    for change in &report.changes {
        println!("  {change}");
    }
    if !apply {
        println!("Dry run. Re-run with --apply to restore.");
    } else if report.verified {
        println!("Restored and verified. Safety snapshot: {}", report.safety_snapshot.unwrap_or_default());
    } else {
        println!("Warning: some changes did not apply; run again to see what is left.");
    }
    Ok(())
}
