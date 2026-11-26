# Depiled

A desktop app for sorting your Steam library into collections, with undo.

If your library has grown into a pile of hundreds of games, Depiled helps you file them:
drag games onto collections, fill collections from filters, or let a few simple rules keep
*Started*, *Unfinished*, *Completed* and *Planned* up to date as you play. You see every
change before it reaches Steam, and you can take any of it back.

It works from the files Steam keeps on your computer. No Steam sign-in, no API key, and no
internet needed unless you want the optional extras.

## What it does

- Shows your whole library as covers or as a sortable table, with demos, software,
  soundtracks and Steam Family games kept apart from your own games.
- Lets you create, colour, rename, duplicate, merge and delete collections, and drag games
  between them.
- Filters by play status, achievements, reviews, Steam Deck and Linux support, price and
  sales, release year, studio, features and tags, and combines anything with *and*, *or* and
  *not* in the advanced builder.
- Fills collections from saved filters, and keeps them topped up.
- Keeps every edit in a draft until you review and save it, with undo, redo and a history of
  earlier versions.
- Optionally adds achievement progress from your public profile, regional prices and sales,
  completion times from [SteamHunters](https://steamhunters.com) and Linux ratings from
  [ProtonDB](https://www.protondb.com). They're fetched in the background, a little at a
  time, and kept on disk so the app works the same offline.

There's a full tour in [docs/features.md](docs/features.md).

## Install

Get the installer for your computer from the
[latest release](https://github.com/Goggwell/depiled/releases/latest):

| Computer | File |
| --- | --- |
| Windows 10 or 11, 64-bit | `Depiled_<version>_x64-setup.exe` |
| Windows 10 or 11, 32-bit | `Depiled_<version>_x86-setup.exe` |
| Mac, Apple silicon | `Depiled_<version>_aarch64.dmg` |
| Mac, Intel | `Depiled_<version>_x64.dmg` |

The installers aren't code-signed yet. On Windows, choose **More info**, then **Run anyway**
if SmartScreen stops it. On a Mac, right-click the app and choose **Open** the first time.

Steam needs to have been signed in on the computer at least once; Depiled finds it on its
own. The [nightly pre-release](https://github.com/Goggwell/depiled/releases/tag/nightly) has
the newest changes between releases.

## How saving works

Steam keeps your collections in a file under its `userdata` folder and rewrites it while it
runs, so Depiled saves with Steam closed. The save dialog closes Steam, writes the changes,
marks them so Steam uploads them to your other devices, and opens Steam again. A copy of
Steam's files is kept before every save, and if Steam changed something in the meantime,
both sets of changes are kept.

Depiled never turns on Steam's debug port, which some tools use to control the Steam client
but which lets any program on the computer do the same.

## Building it yourself

You'll need [Rust](https://rustup.rs) and [Node.js](https://nodejs.org) 20 or newer.

```bash
cd ui
npm install
npx tauri dev      # run the app
npx tauri build    # make an installer
```

Tests and type checks:

```bash
cargo test --workspace
cd ui && npm run check
```

`npm run dev` in `ui/` runs the interface in a browser with made-up games, handy for working
on the UI without Steam.

The core is also a command-line tool: `depiled --help` lists `list`, `plan`, `apply`,
`save`, `undo`, `export`, `import`, `smart`, `watch`, `online` and more.

| Folder | Holds |
| --- | --- |
| `src/` | The Rust core: Steam's file formats, rules and filters, the draft and saving, online extras |
| `ui/src/` | The interface, in Svelte 5 with Tailwind and daisyUI |
| `ui/src-tauri/` | The Tauri desktop shell around the core |
| `tests/` | A large-library benchmark |

[docs/how-it-works.md](docs/how-it-works.md) explains how it's built and why.

## Your data

Settings, rules (`config.toml`) and saved copies live in `%APPDATA%\depiled` on Windows and
`~/Library/Application Support/depiled` on macOS. Your collections themselves stay in
Steam.

## License

MIT
