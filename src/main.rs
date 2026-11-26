use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use depiled::service::{self, OpView, Session};
use depiled::{bundle, config, rules, snapshot};

#[derive(Parser)]
#[command(name = "depiled", about = "Organize Steam library collections, with undo")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List collections as Steam's files hold them
    List,
    /// Summarize the library read from Steam's files
    Library {
        /// Also time a reload and report the size of what the app's window receives
        #[arg(long)]
        timing: bool,
    },
    /// Show what the rules would change. Changes nothing.
    Plan,
    /// Apply the rules and save them to Steam (Steam must be closed; a snapshot is taken first)
    Apply,
    /// Save unsaved changes made in the app (Steam must be closed)
    Save,
    /// Throw away unsaved changes made in the app
    Discard,
    /// Create or inspect the rules file
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Show saved snapshots, newest first
    Snapshots,
    /// Restore collections to a snapshot (newest by default). Shows the plan unless --apply.
    Undo {
        /// Snapshot folder name or prefix, see `snapshots`
        snapshot: Option<String>,
        #[arg(long)]
        apply: bool,
    },
    /// Write a portable backup (collections + rules) to a file
    Export { file: PathBuf },
    /// Restore collections from a backup file. Shows the plan unless --apply.
    Import {
        file: PathBuf,
        #[arg(long)]
        apply: bool,
        /// Also replace the rules file with the one in the backup (the old one is kept)
        #[arg(long)]
        with_config: bool,
    },
    /// Smart collections: create one that fills itself, or top one up
    Smart {
        #[command(subcommand)]
        action: SmartAction,
    },
    /// Show or set how automatic organizing is: off, notify or auto
    Automation { mode: Option<String> },
    /// Watch Steam and organize by the automation setting (Ctrl+C to stop)
    Watch,
    /// Download the online extras that are due: completion times, prices,
    /// achievement progress and ProtonDB ratings
    Online {
        /// Two-letter country for prices, e.g. NL (default: this computer's region)
        #[arg(long)]
        country: Option<String>,
        /// Look everything up again, however recent
        #[arg(long)]
        all: bool,
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
    /// Further conditions as JSON, with all/any/not groups, e.g.
    /// '{"all":[{"field":"publisher","op":"contains","value":"Annapurna"},{"field":"price","op":"lt","value":30}]}'
    #[arg(long = "where")]
    condition: Option<String>,
}

#[derive(Subcommand)]
enum SmartAction {
    /// How many games the criteria match right now. Changes nothing.
    Preview {
        #[command(flatten)]
        criteria: Criteria,
    },
    /// Create a collection filled with matching games, and save it
    Create {
        name: String,
        #[command(flatten)]
        criteria: Criteria,
        /// Remember the criteria so `fill` can top the collection up later
        #[arg(long)]
        auto: bool,
    },
    /// Add games matching a collection's saved criteria, and save
    Fill { id: String },
    /// Show saved criteria by collection id
    List,
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

fn smart_rule(c: &Criteria, session: &Session) -> Result<depiled::smart::SmartRule> {
    let tag_id = |t: &String| {
        t.parse::<u32>()
            .ok()
            .or_else(|| session.tags.iter().find(|(_, n)| n.eq_ignore_ascii_case(t)).map(|(id, _)| *id))
            .with_context(|| format!("Steam has no tag or genre called '{t}'"))
    };
    Ok(depiled::smart::SmartRule {
        played: c.played.clone(),
        achievements: c.achievements.clone(),
        min_review: c.min_review,
        tags: c.tags.iter().map(tag_id).collect::<Result<_>>()?,
        flags: c.flags.iter().map(|f| rules::flag_id(f).with_context(|| format!("unknown flag '{f}'"))).collect::<Result<_>>()?,
        condition: c.condition.as_deref().map(serde_json::from_str).transpose().context("--where isn't a valid condition")?,
    })
}

fn print_ops(ops: &[OpView]) {
    for op in ops {
        let what = match op.kind {
            "rename" => format!("rename to '{}'", op.collection),
            "delete" => format!("delete '{}'", op.collection),
            kind => format!("{kind} {} games: '{}'", op.count, op.collection),
        };
        println!("  {what}");
        if !op.sample.is_empty() {
            let more = op.count.saturating_sub(op.sample.len());
            let tail = if more > 0 { format!(", and {more} more") } else { String::new() };
            println!("      {}{tail}", op.sample.join(", "));
        }
    }
}

/// Saves whatever the session holds, explaining what was written.
fn save(session: &mut Session) -> Result<()> {
    let pending = session.pending();
    if pending.is_empty() {
        println!("Nothing to save.");
        return Ok(());
    }
    print_ops(&pending);
    let report = session.save()?;
    println!("Saved {} changes to Steam. Snapshot: {}", report.changes, report.snapshot.unwrap_or_default());
    if !report.verified {
        println!("Warning: Steam's file doesn't match exactly; run `list` to check.");
    }
    Ok(())
}

/// Shared by `undo` and `import`: show how the collections would change, and save with `apply`.
fn restore_to(target: Vec<depiled::model::Collection>, apply: bool) -> Result<()> {
    let mut session = Session::open()?;
    session.discard();
    session.restore_to("restore", &target, false)?;
    let ops = session.pending();
    if ops.is_empty() {
        println!("Nothing to change; collections already match.");
        return Ok(());
    }
    if !apply {
        print_ops(&ops);
        println!("Dry run. Re-run with --apply to restore.");
        session.discard();
        return Ok(());
    }
    save(&mut session)
}
fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::List => {
            let session = Session::open()?;
            for c in session.saved_collections() {
                let kind = if c.dynamic { " [dynamic]" } else { "" };
                println!("{:<18} {:<30} {} games{kind}", c.id, c.name, c.apps.len());
            }
        }
        Command::Library { timing } => {
            let mut session = Session::open()?;
            if timing {
                let t = std::time::Instant::now();
                session.reload()?;
                let reload = t.elapsed();
                let t = std::time::Instant::now();
                let lib = session.library();
                let json = serde_json::to_vec(&lib)?;
                let tags = serde_json::to_vec(&lib.tags)?.len();
                println!("reload {reload:.0?}, build and encode {:.0?}: {} KB ({} KB tag names)", t.elapsed(), json.len() / 1024, tags / 1024);
            }
            let lib = session.library();
            let with_cover = lib.games.iter().filter(|g| g.cover.is_some()).count();
            let loose = lib.games.iter().filter(|g| g.collections.is_empty()).count();
            let achievements = lib.games.iter().filter(|g| g.achievements.is_some()).count();
            let hunted = lib.games.iter().filter(|g| g.hunters.is_some()).count();
            println!("{} games, {with_cover} with cover art, {loose} in no collection", lib.games.len());
            println!("{achievements} with achievement progress on disk, {hunted} with SteamHunters times");
            let mut kinds = std::collections::BTreeMap::new();
            for g in &lib.games {
                *kinds.entry(g.kind.as_str()).or_insert(0) += 1;
            }
            println!("{}", kinds.iter().map(|(k, n)| format!("{n} {k}")).collect::<Vec<_>>().join(", "));
            let count = |f: &dyn Fn(&depiled::service::GameView) -> bool| lib.games.iter().filter(|g| f(g)).count();
            println!(
                "{} installed, {} Deck verified, {} Deck playable, {} full controller, {} on Linux, {} VR, {} early access, {} from the Steam Family",
                count(&|g| g.installed_bytes > 0),
                count(&|g| g.extras.deck == 3),
                count(&|g| g.extras.deck == 2),
                count(&|g| g.extras.controller == 2),
                count(&|g| g.extras.platforms & 4 != 0),
                count(&|g| g.extras.vr),
                count(&|g| g.extras.early_access),
                count(&|g| g.family),
            );
            for c in &lib.collections {
                let shown = lib.games.iter().filter(|g| g.collections.contains(&c.id)).count();
                let missing = if shown < c.count {
                    let known: std::collections::HashSet<u32> = lib.games.iter().map(|g| g.appid).collect();
                    let ids: Vec<String> = session.collections().iter().find(|x| x.id == c.id).map(|x| x.apps.iter().filter(|a| !known.contains(a)).take(8).map(u32::to_string).collect()).unwrap_or_default();
                    format!("  ({} not in the library: {})", c.count - shown, ids.join(", "))
                } else {
                    String::new()
                };
                println!("  {:<28} {:>4}{}{missing}", c.name, c.count, if c.dynamic { "  (Steam-managed)" } else { "" });
            }
            if lib.draft.pending > 0 {
                println!("{} unsaved changes from the app (save with `save`).", lib.draft.pending);
            }
        }
        Command::Plan => {
            let session = Session::open()?;
            let (ops, notes) = session.preview_rules()?;
            for note in notes {
                println!("Note: {note}");
            }
            if ops.is_empty() {
                println!("Nothing to change.");
            } else {
                print_ops(&ops);
                println!("Dry run. Run `apply` to make these changes (a snapshot is taken first).");
            }
        }
        Command::Apply => {
            let mut session = Session::open()?;
            if session.apply_rules()? == 0 && session.pending().is_empty() {
                println!("Nothing to change.");
                return Ok(());
            }
            save(&mut session)?;
        }
        Command::Save => save(&mut Session::open()?)?,
        Command::Discard => {
            let mut session = Session::open()?;
            let n = session.pending().len();
            session.discard();
            println!("Discarded {n} unsaved changes.");
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
        Command::Snapshots => {
            for (path, snap) in snapshot::list(&snapshot::store_dir()?)? {
                println!("{}  {:<20} {} collections", snap.taken_at, snap.label, snap.collections.len());
                println!("  {}", path.display());
            }
        }
        Command::Undo { snapshot: wanted, apply } => {
            let snaps = snapshot::list(&snapshot::store_dir()?)?;
            let (path, snap) = match &wanted {
                Some(w) => snaps
                    .into_iter()
                    .find(|(p, _)| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(w.as_str())))
                    .with_context(|| format!("no snapshot matching '{w}'"))?,
                None => snaps.into_iter().next().context("no snapshots saved yet")?,
            };
            println!("Snapshot: {}", path.display());
            restore_to(snap.collections, apply)?;
        }
        Command::Export { file } => {
            let session = Session::open()?;
            let config_text = config::load().ok().map(|(_, t)| t);
            bundle::write(&file, session.saved_collections().to_vec(), config_text)?;
            println!("Wrote {}", file.display());
        }
        Command::Import { file, apply, with_config } => {
            let bundle = bundle::read(&file)?;
            restore_to(bundle.collections, apply)?;
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
        Command::Smart { action } => match action {
            SmartAction::Preview { criteria } => {
                let session = Session::open()?;
                let (total, _) = session.preview_smart(&smart_rule(&criteria, &session)?, None)?;
                println!("{total} games match.");
            }
            SmartAction::Create { name, criteria, auto } => {
                let mut session = Session::open()?;
                let rule = smart_rule(&criteria, &session)?;
                let (_, added) = session.create_smart_collection(&name, &rule, auto)?;
                println!("Created '{name}' with {added} games.");
                save(&mut session)?;
            }
            SmartAction::Fill { id } => {
                let mut session = Session::open()?;
                println!("Added {} games.", session.fill_collection(&id)?);
                save(&mut session)?;
            }
            SmartAction::List => {
                for (id, rule) in Session::open()?.smart_rules()? {
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
        Command::Watch => {
            let stop = std::sync::atomic::AtomicBool::new(false);
            let session = std::sync::Mutex::new(Some(Session::open()?));
            println!("Watching Steam. Change the mode with `automation off|notify|auto`.");
            depiled::watcher::run(&session, &stop, &mut |event| match event {
                depiled::watcher::Event::Changed { steam_running } => println!("Steam's files changed (Steam running: {steam_running})."),
                depiled::watcher::Event::Pending { ops, .. } => {
                    println!("Ready to organize:");
                    print_ops(&ops);
                }
                depiled::watcher::Event::Organized { applied, saved, .. } => {
                    println!("Organized automatically: {applied} changes{}", if saved { ", saved" } else { ", will save when Steam closes" });
                }
            });
        }
        Command::Online { country, all } => {
            let settings = service::get_settings()?;
            let country = country.or(settings.region).or_else(depiled::steam::home_region).unwrap_or_else(|| "US".into());
            println!("Prices for {country}.");
            let job = Session::open()?.online_job(&country, all)?;
            let started = std::time::Instant::now();
            // Sources run side by side, so each line says which one it is about.
            let errors = service::run_online(&job, &|task, done, total| {
                if done == total || done % 50 == 0 {
                    println!("{task}: {done}/{total}");
                }
            });
            println!("Took {:.0?}.", started.elapsed());
            for e in errors {
                println!("Problem: {e}");
            }
        }    }
    Ok(())
}
