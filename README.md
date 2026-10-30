# Depiled

Organize your Steam library collections, with undo.

Depiled reads and edits the collections of your running Steam client, files games
automatically by rules you write, and snapshots your library before every change so
nothing can be lost.

- **Rules you name yourself.** "Not played in 7 days" goes to *Paused*, never-started games
  go to *Planned*, fully unlocked games go to *Completed*. Collection names and thresholds
  are yours to set.
- **Smart collections.** A collection can fill itself from filters (played state,
  achievements, review score, Steam flags, tags) and has an "Add matching games" button.
- **Undo everything.** Every change takes a snapshot first. Undo the last step or restore
  any earlier version from History.
- **Automation you control.** Never, tell me, or do it for me.
- **Search, filters, drag and drop,** grid and list views, light and dark themes.
- **Local and offline.** No account, no API key, nothing is sent anywhere.
  Achievement progress comes from the Steam client itself.
- **Fast.** A 25,000-game library filters and sorts in well under 100 ms.

## How it works

Steam can expose its internal browser over a local debug port. Depiled asks once for
permission to switch that on (it creates an empty `.cef-enable-remote-debugging` file in
your Steam folder, which you can delete at any time), then edits collections through the
same code Steam uses itself, so Steam keeps syncing them to the cloud as usual. Steam has to
be restarted once after the permission is set.

## Install

Download the installer for your system from the
[releases page](https://github.com/Goggwell/depiled/releases):

| System | File |
| --- | --- |
| Windows 10/11, 64-bit | `Depiled_*_x64-setup.exe` |
| Windows 10/11, 32-bit | `Depiled_*_x86-setup.exe` |
| macOS, Apple silicon | `Depiled_*_aarch64.dmg` |
| macOS, Intel | `Depiled_*_x64.dmg` |

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
`snapshots`, `undo`, `export`, `import`, `smart`, `automation` and `watch`.

## Layout

| Path | What lives there |
| --- | --- |
| `src/` | Rust core: Steam connection, rules, snapshots, watcher |
| `ui/src/` | Svelte 5 interface (Tailwind and daisyUI) |
| `ui/src-tauri/` | Tauri shell around the core |
| `tests/` | Large-library benchmark (`cargo test --release --test perf -- --ignored`) |

## Where your data lives

Settings, rules and snapshots are kept in `%APPDATA%\depiled` on Windows and
`~/Library/Application Support/depiled` on macOS. Each snapshot also holds a byte-for-byte
copy of Steam's own collection files.

## License

MIT