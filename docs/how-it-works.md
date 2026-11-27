# How Depiled works, and why it's built this way

Questions and answers about the design, roughly in the order someone new to the code would
ask them.

## The idea

**What problem does it solve?**

Steam lets you put games in collections, but doing it by hand for hundreds of games is
slow, and nothing keeps them current: a game you finished last month is still under "Playing
now". Depiled makes filing fast (bulk select, drag, filters, rules), keeps it current as you
play, and does it without risking the data. Every change is previewed, and every save can
be undone.

**Who is it for?**

Two kinds of people. The casual player with 600 games who never organized them wants a
tidy library with no setup. The achievement hunter wants dense data, a sortable table, and
filters like "fewer than 100 achievements and over 90% positive". So the app is simple by
default (four quick filters, one main button) and the power tools are one click away
(advanced builder, table columns, rules file, command line).

**What were the hard constraints?**

- No Steam API key and no sign-in. Asking a casual user to register a developer key is
  where most of them would give up.
- No debug port. The common approach for tools like this is starting Steam with its
  embedded browser's remote-debugging port open and scripting the client through it. It
  works, but while it's open any program on the computer can control Steam. I didn't want to
  ship that.
- Offline first. Everything must work with no connection; online data is a cached extra.
- Every change undoable.

## Architecture

**How is the code laid out?**

Three layers:

1. **A Rust core** (`src/`) that knows Steam's file formats, the rules and filters, the
   draft-and-save model, the snapshots and the online sources. It has no UI and no Tauri
   dependency.
2. **A thin Tauri shell** (`ui/src-tauri/`). Each command is a few lines that lock the
   session, call the core on a blocking thread, and return the result.
3. **A Svelte 5 interface** (`ui/src/`) that renders the data and holds UI state (filters,
   selection, layout).

The core also builds as a command-line tool (`src/main.rs`) with the same capabilities. It
was useful for checking behaviour against my real library without clicking through the UI,
and it's handy for scripting.

**Why Rust and Tauri instead of Electron?**

Most of the work is parsing binary files and writing them back without corrupting anything.
Rust is a good fit for both: strict types for file formats, and no runtime surprises. Tauri
uses the operating system's web view instead of bundling Chromium, so the installer is about
2 MB instead of 100+. On Windows 10 and 11 the WebView2 runtime is already there.

**Why Svelte 5?**

The UI is mostly derived data: the visible list depends on the library, the view, the
search, the filters and the sort. Svelte 5's runes (`$state`, `$derived`) express that
directly, and only what changed recomputes. It also compiles to small, fast code without a
virtual DOM.

**Did you consider Leptos, Dioxus or Yew, so everything is Rust?**

Yes. They wouldn't help: the parts where Rust matters (file formats, network) are already in
Rust. A Rust-to-WebAssembly UI would mean rewriting the interface for no user-visible gain,
and the JavaScript ecosystem (Tailwind, daisyUI, the icon set, fast iteration in a browser
with demo data) is genuinely useful for UI work.

## Reading the library

**Where does the data come from, with no API?**

All from files Steam keeps on disk:

| What | File | Format |
| --- | --- | --- |
| Collections | `userdata/<account>/config/cloudstorage/cloud-storage-namespace-1.json` | JSON list of `[key, entry]` pairs |
| Names, types, tags, features, review scores, Deck status, platforms | `appcache/appinfo.vdf` | Binary key-values with a string table (format version 29) |
| Which apps the account owns, and which come from the Steam Family | `userdata/<account>/config/licensecache` | Encrypted protobuf |
| What each license contains | `appcache/packageinfo.vdf` | Binary key-values |
| Playtime and last played | `userdata/<account>/config/localconfig.vdf` | Text key-values |
| Installed games and sizes | `steamapps/libraryfolders.vdf` | Text key-values |
| Games added by hand | `userdata/<account>/config/shortcuts.vdf` | Binary key-values |
| Achievement progress Steam cached | `userdata/<account>/config/librarycache/<app>.json` and `appcache/stats` | JSON and binary |
| Cover art | `appcache/librarycache/<app>/…` | Images |

`src/vdf.rs` parses Valve's key-value formats, text and binary. `src/appinfo.rs`,
`src/licenses.rs` and `src/local.rs` build the library from them.

**The license cache is encrypted?**

Lightly. It's XOR'd with a stream from the random-number generator in Valve's Source engine,
seeded with the account ID. Once that stream is reproduced (including a warm-up of 40
iterations, which took some trial and error to pin down), the content is a protobuf list of
licenses: the package, some flags, and the owner. Flags filter out things like expired
free weekends. An owner that isn't you means the license is lent through the Steam Family,
which is how Family games get their label.

**What about edge cases?**

A few that came up with real data:

- **Last played without playtime.** A game Steam marks as "last played" with no playtime
  key was installed and never started. That rule fixed games showing up as Started right
  after installing.
- **Unknown apps.** Apps in a collection that Steam knows nothing about locally are kept as
  "Unknown app 1234", so saving never drops them.
- **Kinds.** Demos, playtests, software, tools, soundtracks and videos are told apart by the
  type field in `appinfo.vdf`, and live in their own sidebar sections. Rules only ever look
  at owned games, so a soundtrack never lands in "Planned".

## Writing to Steam

**How do changes get into Steam without the client?**

Steam reads collections from `cloud-storage-namespace-1.json` at start-up. Three details
matter when writing it:

1. **Steam must be closed.** It holds the list in memory and writes it back on exit, so
   anything written while it runs is lost. The save flow closes Steam with
   `steam.exe -shutdown`, writes, and starts it again.
2. **Mark the changes for upload.** Steam uploads every key listed in
   `cloud-storage-namespace-1.modified.json` when it starts. Without that, the next cloud
   sync would bring the old collections back. I confirmed it worked by watching the
   namespace version go up after Steam started.
3. **The second copy.** Some Steam builds also keep the list in their web view's local
   storage (a LevelDB database). When it exists Depiled updates it too, so both agree.
   Opening a LevelDB database writes to it, so reads go through a temporary copy and the
   real one is only touched when saving.

Writes are atomic (write a temporary file, then rename), and after writing, Depiled reads
the file back and compares it with what it meant to write. That's the "verified" flag on a
save.

**What protects the user's data?**

- Before every save, a snapshot: a byte-for-byte copy of Steam's collection files plus a
  readable JSON copy. History restores from these.
- An optional `backup_dir` sends copies to a folder of your choice (for example a synced one).
- Saving is the only operation that touches Steam's files. Everything else edits the draft.

## The draft

**What's the draft, exactly?**

Two lists of collections: `base` (what Steam's files held when last read or saved) and
`draft` (what you want). Every edit changes `draft` only. "Changes not in Steam yet" is the
difference between the two, computed as a list of operations (create, add, remove, rename,
delete) by `ops::plan_restore_exact`. Saving applies those operations to Steam's file.

**Why compute the difference instead of recording each action?**

Because then it's always correct. If you add a game and later remove it, there's nothing to
save. And undo becomes simple: each undo step is a copy of the whole `draft` from before the
edit. Collections are small (a few hundred IDs each), so copying is cheap, and the undo
stack can never get out of step with the data.

**What if Steam changes collections while there are unsaved changes?**

That happens, for example when you add a game to a collection on another computer and it
syncs. On reload, Depiled notices that Steam's files no longer match `base`, and rebases:
it takes Steam's new version, re-applies your changes on top, and tells you both were
kept. It's a three-way merge on sets of game IDs per collection, with renames and deletes
resolved per collection.

**Does the draft survive a restart?**

Yes. It's written to `draft.json` in Depiled's folder after every edit, and removed once
there's nothing pending.

**How do edits stay instant with big libraries?**

An edit doesn't reload the library. The Tauri command compares which collections each game
was in before and after, and returns a small patch: the new collection list, the draft
state, and the membership of only the games that moved. The window applies it to the data
it already has.

## Several accounts

**How does it handle more than one Steam account?**

Steam keeps each account's files under `userdata/<account id>`, and Depiled treats every
folder with a license list as an account with a library. Names and profile pictures come from
Steam's `config/loginusers.vdf` and `config/avatarcache`. The account to show is the one
picked in the app if there is one; otherwise whoever is signed in right now (the registry's
`ActiveProcess\ActiveUser` on Windows, `registry.vdf` elsewhere); otherwise whoever signed in
last. The watcher re-checks that every few seconds, so when someone else signs in, the
library follows.

Depiled's own files for an account (the draft, snapshots, rules, auto-fill criteria,
colours, profile achievement progress) live in `accounts/<account id>` in its folder. Settings
and the online caches that are the same for everyone stay at the top. Achievement progress is
addressed by the account it was fetched for, so a refresh still running when you switch can't
write into the other account's file.

**What happened to files from versions that only knew one account?**

They're moved into an account folder the first time this version runs. To pick the right
account, Depiled compares the collection ids those files mention (in the draft, colours and
newest snapshot) with each account's collections in Steam. The ids are random, so a match is
no accident. Only if nothing matches does it fall back to the account Steam wrote to last,
which is how older versions chose.

## Collection order

**How do you get Steam to keep an order it sorts alphabetically?**

By the names. With **Keep my own order** on, every collection gets a two-digit number and a
space in front ("01 Playing"), three digits past 99, renumbered after every edit so there are
no gaps and new collections join the end. Zero-padding means the order holds whether Steam
compares names as text or numbers. Everywhere Depiled matches names (rules, auto-fill,
presets, duplicate checks) it compares them without the number, so "03 Playing" is still
Playing. The window shows the plain names in number order, and a rename keeps the number.
Only two or three digits followed by a space count, so a collection called "7 Days to Die"
is left alone.

## Watching Steam

**How does Depiled notice you played something?**

`src/watcher.rs` checks the modification times of the files that change when you play
(`localconfig.vdf`, the achievement caches, the collection file) every 5 seconds. That's a
handful of `stat` calls, not a file read. When something changes, it waits until things have
been quiet for 8 seconds, so a game launch, a session and an exit don't trigger three
reloads, then tells the window. A full check runs every 10 minutes as a safety net. What
happens next depends on the automation setting: nothing, a banner offering the changes, or
applying them and saving the next time Steam is closed.

## Rules and filters

**How do the rules work?**

A rule names a collection and some conditions. Rules can share a group, which makes them
exclusive: the first matching rule in the group wins, and the game is taken out of the
group's other collections. That's how Completed, Unfinished, Started and Planned behave like
one status. Two more options: a *sticky* rule never moves games out once they're in
(finished games stay finished), and *sync* on an ungrouped rule removes games that stop
matching. Rules live in `config.toml`, which is plain text with comments, so power users can
edit it and casual users never need to.

**And the advanced filters?**

A condition is a small JSON tree:

```json
{ "all": [
  { "field": "achievements", "op": "lt", "value": 100 },
  { "field": "review_percent", "op": "gt", "value": 90 },
  { "any": [
    { "field": "price", "op": "lt", "value": 30 },
    { "field": "on_sale", "op": "is", "value": true }
  ] },
  { "not": { "field": "tags", "op": "has_any", "value": [1667] } }
] }
```

`all`, `any` and `not` nest freely, and a leaf tests one of about 30 fields with an operator
that fits the field's type (text, number, choice, yes/no, or set). The same tree is the
filter in the window, the saved auto-fill rule, and an extra condition on a `config.toml`
rule.

**Why is the matching logic written twice, in Rust and TypeScript?**

The window needs instant counts as you click filters ("95 games match"), so it can't make a
round trip per click. The core needs the same logic when it applies rules, including when
the window isn't open. So both implement it: `src/cond.rs` and `ui/src/lib/criteria.ts`. The Rust
tests hold the reference cases, and the window's version was checked against the same 17
conditions on the same sample game. Making that check an automated test on the TypeScript
side is an obvious next step.

**What are the rules about unknown values?**

An unknown value never matches a comparison. A game with no known price doesn't match
"price under 30", and a game never played doesn't match "last played more than 0 days ago".
Otherwise filters quietly include games you didn't mean. *is known* and *is unknown* ask
about it explicitly.

**How fast is filtering?**

In the window, each rule is compiled once into a chain of small functions. Field lookups,
lower-casing, number parsing and value sets are all prepared up front, so checking a game
runs only the comparisons. A nested five-condition rule runs over 20,000 games in about
3.5 ms. Compiled rules are cached per rule object, and every filter change produces a new
object, so the cache can't go stale. On the Rust side, facts are borrowed from the game
rather than copied, so evaluating rules over a library allocates almost nothing.

## Online extras

**How do you get achievement progress without an API key?**

From the user's own public Steam profile. The achievements page,
`steamcommunity.com/profiles/<id>/achievements/<app>`, renders server-side with the data
embedded as JSON in a `<script id="valve-ssr-data">` tag. Depiled reads the player's
unlocked list and the game's schema from it. The per-player API that would be cleaner
needs a key, and tools that read stats through the Steam client mark the game as running,
which I didn't want. Two lessons from real pages:

- That address answers directly. The older `/stats/<app>/achievements/` address redirects
  twice first, which tripled the round trips.
- A profile with private game details still returns the page, with an empty list. Without
  noticing that, it would be read as "0 unlocked" for every game. Now an empty list counts
  as "nothing to read", and three in a row means the profile is private.

**It's one page per game. Isn't that a lot?**

It's the biggest cost, about 56 KB compressed per page, so it's the most carefully
scheduled. A game's page is fetched only when something can have changed:

- it's never been read, unless Steam already left fresh progress on disk;
- it's been played since the last read, on any computer (total playtime grew);
- SteamHunters reports a different number of achievements (an update added some);
- or as a slow catch-all, every six to eight months.

Games already at 100% aren't refetched after playing, since there's nothing left to
unlock. Progress Steam writes to disk after a play session is used directly, and wins if
it's newer than the profile.

**And the other sources?**

| Source | Request | Batch | Refresh |
| --- | --- | --- | --- |
| Steam store (`IStoreBrowseService/GetItems`) | prices, sale end dates, review counts, release dates | 100 games per request | weekly, per region |
| SteamHunters (`/api/apps`) | time to 100%, players, achievement count | 100 games per request | when Steam reports the game changed, monthly otherwise, new games right away |
| ProtonDB (`/api/v1/reports/summaries/<app>.json`) | Linux rating | one per game, four at a time | about monthly, less for settled ratings and games with no reports |

All of them are keyless public endpoints.

**Can't it skip data that hasn't changed?**

Only if something says so before the download. Comparing a fresh answer with the cache
saves nothing, because by then it's been downloaded. So Depiled uses two kinds of "nothing
changed" signal:

- **From the server.** ProtonDB sends ETags (see below). I checked the others: the store,
  SteamHunters and the profile pages send no ETag or Last-Modified, and none of them takes
  an "updated since" parameter.
- **From Steam itself.** Steam's `appinfo.vdf` keeps a change number for every app, which
  goes up whenever its store data, a branch or a depot changes. Depiled records the number
  when it asks SteamHunters about a game and asks again only once it moves (at most daily),
  with a monthly catch-all for completion times drifting as more people play. A game update
  that adds achievements moves the number, so new achievements are still caught right away.
  On a quiet week that skips most of the library without a single request.

**How is "due" decided without everything expiring at once?**

Each periodic refresh adds a per-game offset from a hash of the app ID (SplitMix64), so a
library fetched together on day one spreads its later refreshes across weeks instead of
re-downloading everything on the same day. The first version used a plain multiply, and
round-numbered app IDs bunched onto a few days, hence the better hash.

**What makes the network side efficient?**

- **One HTTP client for the app.** Connections stay open, so a run of requests to the same
  site costs one TLS handshake. Responses are gzip-compressed.
- **Sources run in parallel, with a pace per site.** There are three lanes: the store;
  SteamHunters then your profile; and ProtonDB. Each site has its own minimum gap between
  request starts, measured from start to start, so a slow answer isn't followed by a full
  pause as well. A first run takes as long as the slowest lane instead of the sum.
  SteamHunters goes first in its lane because its achievement counts decide which profile
  pages are worth asking for.
- **A little concurrency where it's cheap.** ProtonDB's ratings are tiny files on a CDN, one
  per game, so four lookups run at once, still sharing one pace (150 ms between starts).
  On a first run that's about twice as fast: 200 ratings took 30 seconds. The other
  sites stay one request at a time.
- **Retries for passing failures.** Timeouts, dropped connections, server errors and "429
  Too Many Requests" are tried again, up to three tries, after 2 then 8 seconds or as long
  as the site's `Retry-After` asks. A 429 also doubles that site's gap for the rest of the
  run, so it isn't hit again straight away. Anything else, like a 404, is an answer, not a
  failure.
- **Typed, lean parsing.** Answers are read into structs with only the fields Depiled uses.
  The profile page's data block holds every achievement's name, description and icon; the
  parser borrows from the page text and skips all of that without building it, and only
  counts achievements in the schema. Numbers are read whether Steam sends them as numbers or
  as text, so one odd field doesn't fail a whole batch.
- **Revalidation where the server supports it.** ProtonDB sends ETags, so a recheck sends
  `If-None-Match` and an unchanged rating comes back as "304 Not Modified" with no body.
  The Steam store and SteamHunters answers don't offer usable validators, so they rely on
  the schedule instead.
- **Stoppable and resumable.** A global cancel flag is checked between requests, and caches
  are saved every few batches and on error or cancel, so stopping keeps what arrived.
- **Fresh data costs nothing.** A second run with nothing due makes zero requests.
- **Usage is shown.** Requests and estimated bytes are counted per month and shown in
  Settings.

**What about regions?**

Prices are per region, so the store cache is keyed by country. Switching region moves the
current data aside instead of throwing it away, which makes switching back instant (0.1 s)
and switching to a new region a normal fetch (about 30 s for a large library).

**How does the window find out what's happening?**

The fetch runs on a background thread and sends progress events, added up across lanes
("achievement progress and ProtonDB ratings, 140 of 900"). The app stays fully usable
meanwhile. When it finishes, the window asks for an online-only reload: the cached online
data is laid over the library again, but Steam's files aren't re-read, since they haven't
changed.

## The interface

**How does it stay fast with a huge library?**

- **Windowing.** The grid and the table draw only the rows on screen plus a few either side.
  Rows have fixed heights, so every row's position is known without measuring, and
  scrolling 25,000 games costs the same as scrolling 200.
- **No deep reactivity on big data.** The library is held in `$state.raw`. It's replaced as
  a whole and never edited in place, so Svelte doesn't wrap thousands of games in reactive
  proxies.
- **A lean payload.** The core sends the window only the store fields it shows, and cover
  paths are looked up once and remembered (finding them takes a folder listing per game).
  Building and encoding the library went from 165 ms to 18 ms.

**Why does the design look the way it does?**

It's a tool people operate, so precision and calm over decoration:

- **Covers carry the colour.** The app around them is a quiet slate, in light and dark.
- **Amber means one thing:** changes that aren't in Steam yet. It's on the status bar, the
  save button and the dots on changed collections, and nowhere else, not even the focus
  ring (that's ink). If you see amber, you have something to save.
- **Two typefaces, each with one job.** Atkinson Hyperlegible Next for everything you read
  and press: it was designed for legibility, so I, l and 1 or O and 0 can't be confused in a
  dense list of names and numbers, and its tabular figures keep table columns aligned.
  Bricolage Grotesque, with its optical-size axis, for titles, where some character helps.
  Both ship inside the app.
- **Collections are shelves with coloured labels.** The colour mark appears in the sidebar,
  under covers, in table rows and next to the title, so a collection is recognizable
  anywhere.
- **Plain words.** "Take 3 games out of Unfinished", not "Remove from collection". The voice
  says what happens and doesn't apologize.

The design went through a structured critique: two independent reviews, one by reading
and using the app, one by an automated scan. It scored 25/40. The fixes that came out of it:

- the table always keeps the name column;
- the details panel only appears when something is picked;
- the focus ring stopped using amber (it was also too low-contrast);
- the filter wall was split into common filters plus *More filters*;
- the collection menu became keyboard-reachable;
- saving got a clear ending.

**Can the UI be worked on without Steam?**

Yes. In a plain browser (`npm run dev`), the app detects it's not inside Tauri and uses a
mock backend with invented games and covers. `?games=10000` makes a big library for
performance work, and `?theme=dark` or `?theme=light` forces a theme for screenshots.

## Feedback and crashes

**How do reports reach you without your email being in the app?**

The app posts a report (kind, message, optional reply address, technical details) as JSON to a
form service that forwards it by email. The service's address is compiled in from the
`DEPILED_FEEDBACK_URL` build variable, which the release workflows fill from a repository
secret, so neither it nor the inbox behind it is in the public code. A build without it, like
one made from source, offers **Copy report** instead.

**What happens on a crash?**

The release build uses `panic = "abort"`, so any panic ends the program. A panic hook in
`src/report.rs` runs first: it writes a crash log (the message, where it happened, the
thread, Windows and Depiled versions, the time and a backtrace) to `crashes/` in Depiled's
folder, then starts the app again with `--crash-report <file>`. That second copy shows only a
small "Depiled closed unexpectedly" window. If it can't start, the main window finds the log
on its next start and asks there. Sent or dismissed logs move to `crashes/handled`, and only
the last 20 are kept. Unsaved changes are on disk already, so a crash doesn't lose them.

Every report and log goes through `scrub`, which blanks the home folder (and so the Windows
user name) and Steam ids before anything is shown or sent. Setting `DEPILED_CRASH_TEST=1`
makes the app crash four seconds after starting, to try the reporter.

## Testing and shipping

**How is it tested?**

- **Rust unit tests** cover the parts where a mistake would hurt: the file formats (VDF,
  the license cache), the draft diff and rebase, the rule groups, the condition engine, and
  each online source's parser and scheduling. They use small hand-written fixtures modelled
  on real files.
- **A benchmark test** builds a 25,000-game library.
- **The interface** is type-checked with `svelte-check`.
- **On a real library.** Behaviour was checked against a real library of about 670 entries:
  saving, syncing, the online sources, and region switching.

**How are releases built?**

GitHub Actions:

- **Every push:** tests and type checks on Windows and macOS.
- **Every push to main:** a nightly pre-release is rebuilt, and its installers stay
  downloadable for 30 days.
- **A version tag:** builds the Windows installers (64- and 32-bit) and attaches them to a
  release. The macOS builds are paused until they can be tested on a Mac; the code and the
  macOS CI run stay, so turning them back on is a matter of uncommenting them in the
  workflows.

**How does the app update itself?**

With Tauri's updater. Both workflows sign the update files with a private key kept as a
repository secret (only the workflows make update files, through `tauri.updates.conf.json`,
so building from source needs no key), and attach a `latest.json` that lists each platform's file and
signature. The app has the matching public key built in. It fetches `latest.json` from the
newest release (or from the nightly pre-release, if you chose nightly builds), and if
there's something newer, downloads it while reporting progress, checks the signature and
installs it. On Windows that runs the installer in its quiet mode, which restarts the app.
(On macOS it would swap the app bundle and restart.)

Nightly builds are numbered after the next release (`0.2.1-nightly.57` after 0.2.0), so
each one is distinct. On the nightly channel, any build that isn't the one running counts
as newer, since the nightly pre-release always holds the newest build of main.

Two details: the install waits for the app's state lock, so it can't land halfway through a
save to Steam; and unsaved changes are already on disk, so they survive the restart.

## Trade-offs and what's next

**What would you do differently, or next?**

- **Saving needs Steam closed.** That's the price of not using the debug port. Depiled
  makes it a single click, but it does close any game that's running.
- **Code signing.** The installers aren't signed yet, so Windows warns on first launch.
- **Private profiles** only get the achievement progress Steam leaves on disk. That's the
  honest limit of not using an API key.
- **The duplicated matcher.** It is a maintenance cost; the shared test cases keep it in
  check. If it grew much more, I'd consider compiling the Rust matcher to WebAssembly for
  the window.
- **macOS** builds are paused until they've been tested on a real Mac.
