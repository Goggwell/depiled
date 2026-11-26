# Depiled

Organize your Steam library collections, with undo.

Depiled reads your library straight from Steam's files, lets you sort games into
collections by hand or by rules you write, and saves the result back to Steam. Every save
is snapshotted first, so nothing can be lost.

- **Rules you name yourself.** "Not played in 7 days" goes to *Paused*, never-started games
  go to *Planned*, fully unlocked games go to *Completed*. Collection names and thresholds
  are yours to set.
- **Smart collections.** A collection can fill itself from filters (played state,
  achievements, review score, Steam flags, tags) and has an "Add matching games" button.
- **Edit freely, then save.** Changes stay in the app until you review and save them to
  Steam. Undo and redo any step, or restore an earlier version from History.
- **Tidy collections.** Colour, rename, duplicate and merge collections.
- **Automation you control.** Never, tell me, or do it for me.
- **Search, filters, drag and drop,** grid and list views, light and dark themes.
- **No account or API key.** The library, playtime, tags, Steam Deck status and install sizes
  come from Steam's own files. Optional details from the internet, each fetched in the
  background and kept on disk so the app works the same offline:
  - achievement progress, read from your public Steam profile (Game details must be public)
    for games you've played: again after you play, when an update adds achievements, and
    every six to eight months;
  - prices, sales and review counts for your region (detected from your computer, or picked
    in Settings), weekly; regions used before switch instantly;
  - completion times from [SteamHunters](https://steamhunters.com), checked weekly;
  - Linux and Steam Deck ratings from [ProtonDB](https://www.protondb.com), each game about
    every 30–45 days, or every 90–135 days for established ratings.

  The first download depends on library size and which sources are enabled. Settings shows
  the request count and estimated data downloaded this month.
- **More than games.** Demos, playtests, software, soundtracks and games you added to Steam
  by hand show up too, each in its own place. Rules and auto-fill only ever touch games.
- **Fast.** A 25,000-game library filters and sorts in well under 100 ms.

## How it works

Steam keeps collections in `userdata/<account>/config/cloudstorage/cloud-storage-namespace-1.json`.
Depiled reads that file (and Steam's app, license and playtime caches) any time. Saving
needs Steam closed, because Steam rewrites the file while it runs: the save dialog closes
Steam, writes the changes, marks them for upload so Steam syncs them to your other devices,
and opens Steam again. If Steam changed your collections in the meantime, both sets of
changes are kept. Some Steam versions keep a second copy of the list in their browser
storage; when it exists it is updated too.

Achievement progress is only on disk for games Steam has cached on this computer, so the
"every achievement" rule only counts those.
## Install

Download the installer for your system from the
[releases page](https://github.com/Goggwell/depiled/releases):

| System | File |
| --- | --- |
| Windows 10/11, 64-bit | `Depiled_*_x64-setup.exe` |
| Windows 10/11, 32-bit | `Depiled_*_x86-setup.exe` |
| macOS, Apple silicon | `Depiled_*_aarch64.dmg` |
| macOS, Intel | `Depiled_*_x64.dmg` |

Want the newest changes before a numbered release? The
[nightly pre-release](https://github.com/Goggwell/depiled/releases/tag/nightly) is rebuilt on
every change, and each build's installers stay downloadable for 30 days from the
[Actions tab](https://github.com/Goggwell/depiled/actions/workflows/nightly.yml).

The Windows installers need no admin rights. The macOS build is new and has seen less
testing than the Windows one.

The installers are not code-signed yet, so Windows SmartScreen and macOS Gatekeeper will
warn on first launch.

## Build from source

You need [Rust](https://rustup.rs) and [Node.js](https://nodejs.org) 20 or newer.

```bash
cd ui
npm install
npx tauri dev      # run the app
npx tauri build    # produce an installer
```

Core tests and the type check:

```bash
cargo test --workspace
cd ui && npm run check
```

### Command line

The core is also a CLI. `depiled --help` lists everything: `list`, `plan`, `apply`,
`save`, `discard`, `snapshots`, `undo`, `export`, `import`, `smart`, `automation`, `watch`
and `online`.

## Layout

| Path | What lives there |
| --- | --- |
| `src/` | Rust core: Steam file formats, rules, draft and save, snapshots, watcher |
| `ui/src/` | Svelte 5 interface (Tailwind and daisyUI) |
| `ui/src-tauri/` | Tauri shell around the core |
| `tests/` | Large-library benchmark (`cargo test --release --test perf -- --ignored`) |

## Where your data lives

Settings, rules and snapshots are kept in `%APPDATA%\depiled` on Windows and
`~/Library/Application Support/depiled` on macOS. Each snapshot also holds a byte-for-byte
copy of Steam's own collection files.

## License

MIT