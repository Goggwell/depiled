# Depiled: features and installation

Depiled is a desktop app for organizing Steam library collections, the shelves Steam shows in
its Library view. It reads your library from Steam's files on your computer, lets you sort
games by hand or by rules, shows you exactly what will change, and saves it back to Steam.
Every change can be undone.

It runs on Windows 10 and 11 (64-bit and 32-bit). The code supports macOS too, but there are
no Mac builds until it has been tested on a real Mac.

## Contents

- [Installing](#installing)
- [First start](#first-start)
- [Your library](#your-library)
- [Several Steam accounts](#several-steam-accounts)
- [Collections](#collections)
- [Filters](#filters)
- [Auto-fill and rules](#auto-fill-and-rules)
- [Saving to Steam](#saving-to-steam)
- [Undo and history](#undo-and-history)
- [Details from the internet](#details-from-the-internet)
- [Feedback and crash reports](#feedback-and-crash-reports)
- [Keyboard](#keyboard)
- [Privacy and safety](#privacy-and-safety)
- [Command line](#command-line)
- [Limits worth knowing](#limits-worth-knowing)

## Installing

1. Open the [releases page](https://github.com/Goggwell/depiled/releases) and pick the newest
   release.
2. Under **Assets**, download the file for your computer:

   | Your computer | File |
   | --- | --- |
   | Windows 10 or 11, 64-bit (almost every PC) | `Depiled_<version>_x64-setup.exe` |
   | Windows 10 or 11, 32-bit | `Depiled_<version>_x86-setup.exe` |

3. Install it.
   - **Windows:** run the installer. It doesn't need admin rights. Windows SmartScreen may
     say it "protected your PC" because the app isn't code-signed yet; choose **More info**,
     then **Run anyway**.
4. Make sure Steam has been signed in on this computer at least once. Depiled finds Steam
   and your account on its own; there's nothing to configure.

The installer is about 2 MB. Depiled uses the system's built-in web view (WebView2 on
Windows, which comes with Windows 10 and 11), so it doesn't ship its own browser.

**Updating.** You only download Depiled once. When a new version is out, an **Update to
<version>** button appears in the status bar; it downloads the update with a progress bar,
installs it and restarts Depiled. Changes you haven't saved to Steam yet are still there
afterwards. Under **Settings → Updates** you can check by hand, turn off the check at
startup, and choose what to get:

- **Releases:** numbered versions.
- **Nightly builds:** a build made after every change to the code, numbered after the next
  release (for example `0.2.1-nightly.57`). You get fixes first, with less testing.
  Switching back to Releases offers the next numbered version once it's out.

Every update is signed when it's built, and Depiled won't install one whose signature
doesn't match. (That's separate from Windows code signing, which the installers don't have
yet.) Versions before 0.2.1 can't update themselves, so install 0.2.1 by hand
once.

Nightly installers are also on the
[nightly pre-release](https://github.com/Goggwell/depiled/releases/tag/nightly), and each
build stays downloadable for 30 days from the
[Actions tab](https://github.com/Goggwell/depiled/actions/workflows/nightly.yml).

**Uninstalling.** Use Windows' **Installed apps**. Your
collections stay in Steam. Depiled's own settings and saved copies are in
`%APPDATA%\depiled`; delete
that folder to remove them too.

## First start

Depiled opens straight onto your library. A card at the top asks two questions once:

- **Sort games as you play?** When you start, stop or finish a game, Depiled can move it to
  Started, Unfinished or Completed. Choose *Ask me first*, *Do it for me*, or *No*.
- **Add details from the internet?** Achievement progress, prices and sales, and completion
  times. *Stay offline* keeps everything local.

Both can be changed later in Settings.

## Several Steam accounts

If more than one Steam account uses the computer, Depiled shows one library at a time and
keeps the others apart.

- **Whose library.** By default Depiled follows Steam: it shows whoever is signed in, and
  switches by itself when someone else signs in. With Steam closed, it shows whoever signed in
  last.
- **Switching by hand.** The account at the bottom of the sidebar opens a list of every
  account with a library on this computer, with Steam's profile picture and name. Picking one
  sticks until you choose **Follow Steam's sign-in** again, so you can organize an account
  that isn't signed in.
- **Each account keeps its own** unsaved changes, history, rules, auto-fill criteria,
  collection colours and achievement progress. Switching never loses anything: unsaved
  changes wait for you, and the list shows how many each account has. Settings, and details
  that are the same for everyone (prices, ratings, completion times), are shared.
- **Saving an account that isn't signed in** works the same way. Steam picks up the changes,
  and syncs them to its cloud, the next time that account signs in.


- **Everything Steam knows about.** Games, demos, playtests, software, tools, soundtracks
  and videos, each in its own place in the sidebar. Games you added to Steam yourself
  (non-Steam shortcuts) appear with your games.
- **Steam Family games** are marked with a blue *Family* label. They're lent by someone in
  your Steam Family, so rules and auto-fill leave them alone.
- **Three ways to look at it:** large covers, small covers (about twice as many on screen),
  or a table. The table has a column picker with 16 columns:
  - play: time played, last played;
  - reviews and scores: reviews, Metacritic, review count;
  - achievements: achievements, time to 100%;
  - the game: release date, developer, type;
  - platforms: Steam Deck, ProtonDB;
  - your computer: size on disk, the collections a game is in;
  - the store: price and sale.

  If the window is too narrow for every column you picked, the name keeps its room and the
  header shows how many columns are left out.
- **Search** matches names, studios and tags as you type.
- **Sorting** by name, recently played, most played, reviews, Metacritic, release date, time
  to 100%, size, developer, price, sale or review count. Click a table heading to sort by
  it; click again to flip the order. Numbers inside names sort as numbers ("Part 2" comes
  before "Part 10").
- **The details panel** opens when you pick a game. It shows the game's collections (tick
  boxes to file it), playtime, reviews, Steam Deck status, platforms, size on disk, ProtonDB
  rating, store price and sale, achievement progress, completion times and tags. Pick several
  games and it becomes a bulk panel: add them all to a collection, take them out of the one
  you're in, or make a new collection from them.
- **Covers** come from Steam's own cache, so nothing is downloaded for them. Under each
  cover: playtime, and achievement progress (for example 6/20) for games in progress. A
  trophy marks games with every achievement unlocked, and coloured bars show which of your
  colour-coded collections a game is in.
- **Large libraries are fine.** Only the rows on screen are drawn, so 25,000 games scroll as
  smoothly as 200.

## Collections

- **Make one** with the **+** next to *Collections*: start empty, use the games you picked,
  or fill it from filters.
- **Add games** by dragging them onto a collection in the sidebar. Hold **Shift** while
  dropping to move them out of the collection you're viewing instead of copying.
- **The collection menu** (the **⋯** button that appears on hover, or right-click):
  - Colour: eight colours or none. The colour shows in the sidebar, under covers and next to
    the title.
  - Rename (or double-click the name).
  - Duplicate.
  - Merge others into this one: pick the collections to fold in, and Depiled shows how many
    games the result will hold.
  - Auto-fill (see below).
  - Delete: the games stay in your library, and Undo brings the collection back.
- **Steam's dynamic collections** (the ones Steam fills itself) are shown with a lock and
  can't be changed.
- **Not in a collection** lists every game you haven't filed yet: a good place to start.

## Filters

Open **Filters** in the toolbar. The count on the button shows how many are on, and each
active filter appears as a small label under the toolbar that you can click to remove.

**Quick filters** cover the common cases first:

- played: any, never played, played, or not played in 7+ days;
- achievements: all unlocked, or in progress;
- installed or not;
- on sale or not.

**More filters** opens the rest:

- review score, and time to unlock every achievement;
- has achievements, VR, early access, Family game;
- release years, developer or publisher;
- highest price, fewest reviews;
- Windows, macOS or Linux;
- Steam Deck rating, ProtonDB rating;
- Steam features (single-player, co-op, controller support and so on);
- genres and tags, searchable.

**Advanced** builds any combination with *and*, *or* and *not*: "match all of these", "any
of these" or "none of these", with groups inside groups. Every field above is available,
plus:

- name, developer and publisher text;
- playtime, last played (in days), achievement counts and percentage;
- size on disk;
- the collections a game is in.

Some examples:

- Publisher contains *Annapurna*.
- Last played between 365 and 1,095 days ago.
- Fewer than 100 achievements, *and* over 90% positive reviews, *and* price under 30.
- (On sale *or* price under 10) *and* none of (Horror, Roguelike).

Unknown values never count as a match: a game with no known price never matches "price
under 30". Use *is known* or *is unknown* to ask about that directly. Quick and advanced
filters add up; the Advanced tab tells you when quick filters are also applied.

**Save as collection** turns the current filters into a new collection in one step.

## Auto-fill and rules

**Auto-fill** gives a collection a saved set of filters. A bar above the collection says
what it fills itself with ("Fills itself with games that are never played, reviews 80% or
better") and offers **Add 12 matches** whenever new games fit. You can edit the filters or
turn auto-fill off from that bar or the collection menu.

**Auto-organize** (the main toolbar button) runs your rules over the whole library and shows
the result as a list of plain sentences, for example "Add 18 games to Unfinished" with a
few of the game names, before anything happens. The starting rules are:

| Collection | Gets games that… |
| --- | --- |
| Completed | have every achievement unlocked (and stay there) |
| Unfinished | were played, but not in the last 7 days |
| Started | were played recently |
| Planned | have never been launched |

These four form a group: a game sits in only one of them, and moves as you play. Rules live
in `config.toml` in Depiled's folder, where you can rename the collections, change the
7-day threshold, add your own rules (by tags, features, reviews, playtime, or any advanced
condition), and keep games you've filed elsewhere out of a rule.

**Organize as I play** (Settings) decides what happens when Depiled notices you played
something while it's open:

- *Never*: you press Auto-organize yourself.
- *Ask me*: a banner offers the changes.
- *Do it for me*: the changes are made and saved the next time Steam is closed.

## Saving to Steam

Nothing you do in Depiled touches Steam until you save. While there are unsaved changes:

- the status bar at the bottom turns amber and counts them;
- collections with changes have an amber dot;
- **Review and save** (amber) opens a preview.

The preview lists every change in words, with sample game names. Steam rewrites its
collection file while it runs, so if Steam is open, Depiled offers to close it, save, and
open it again. Afterwards the status bar keeps "Saved 3 changes to Steam at 14:02" until
your next change. Steam then syncs the collections to your other devices as usual.

If Steam changed your collections while Depiled had unsaved changes (say, you added a game
on another computer), both sets of changes are kept.

## Undo and history

- **Undo and redo** (Ctrl+Z, Ctrl+Shift+Z) cover every edit, including merges, deletes,
  auto-fill and Discard. Unsaved changes survive closing and reopening the app.
- **History** keeps a copy of your collections from just before every save. Each entry says
  when, and how many collections and games it held. Restoring one puts it in your unsaved
  changes, so you can look before saving it back.

## Details from the internet

All optional, and all switchable one by one in Settings. Everything fetched is kept on your
computer, so the app works the same with no connection.

| Detail | Comes from | Checked |
| --- | --- | --- |
| Achievement progress | your public Steam profile | after you play a game, when an update adds achievements, and otherwise every six to eight months |
| Prices, sales, sale end dates, review counts | the Steam store, for your region | weekly |
| Time to unlock every achievement | [SteamHunters](https://steamhunters.com) | when Steam reports the game changed, monthly otherwise, and right away for new games |
| How well it runs on Linux and Steam Deck | [ProtonDB](https://www.protondb.com) | about monthly; settled ratings every three months or so |

- **Region.** Prices follow your computer's region, or one you pick in Settings. Prices for
  regions you've used before are kept, so switching back is instant.
- **Progress.** A status line at the bottom shows what's being fetched and how far it is,
  with a panel listing each source's last update. You can stop a fetch at any time and keep
  what arrived.
- **Data use.** Settings shows this month's request count and estimated download.
- **Achievement progress needs Game details set to Public** in Steam's privacy settings;
  Depiled says so if it isn't. It also reads progress Steam leaves on your computer after a
  play session, which needs no internet at all.

## Feedback and crash reports

- **Send feedback.** The speech-bubble button in the toolbar opens a short form: say whether
  something's wrong, you have an idea, or something else, write it, and optionally leave an
  email address for a reply. It goes straight to the developer.
- **Report an error.** Error messages have a **Report** button that opens the same form with
  the error already filled in.
- **When Depiled crashes,** a small window opens straight away, like a game's crash reporter:
  it says what happened, lets you add what you were doing, and sends the crash log if you
  press **Send report**. If that window can't open, Depiled asks the next time it starts.
  Changes you hadn't saved to Steam are kept either way.
- **What's in a report** is shown before you send it: what you wrote and, if you leave it
  ticked, Depiled's and Windows' versions, how many games and collections you have, a few
  settings and recent error messages. Never your Steam account, names or library, and your
  user folder and Steam ids are blanked out of every message. Nothing is sent unless you
  press Send.
- **Can't send?** **Copy report** puts the whole report on the clipboard, to paste into an
  issue on GitHub or anywhere else.

## Keyboard

| Keys | Does |
| --- | --- |
| Arrow keys | Move between games |
| Ctrl+click, Shift+click | Pick several games, or a run of them |
| Ctrl+A | Pick everything shown |
| Esc | Clear the selection |
| Delete | Take picked games out of the open collection (the next game is picked, so you can keep going) |
| Ctrl+Z, Ctrl+Shift+Z | Undo, redo |
| Ctrl+S | Review and save |
| Shift+drag | Move instead of add |

The keyboard button in the toolbar lists these too.

## Privacy and safety

- No Steam sign-in, no API key, no account of any kind.
- Depiled doesn't use Steam's debug port. Some tools turn it on to control Steam, and while
  it's on, any program on the computer can do the same. If an older version of Depiled
  switched it on, Settings offers to switch it off.
- With details from the internet off, nothing about your library leaves your computer. With
  them on, Depiled contacts the Steam store, your public Steam profile page, SteamHunters and
  ProtonDB, and nothing else. The update check asks GitHub for the newest version and sends
  nothing about you; it can be turned off in Settings. Feedback and crash reports are only
  sent when you press Send, and show what they contain first.
- Before every save, a copy of Steam's own collection files is kept. You can also point
  `backup_dir` in `config.toml` at a synced folder for off-computer backups.

## Command line

The installed app is the easiest way in, but the same engine is a command-line tool for
scripting. `depiled --help` lists the commands: `list`, `library`, `plan`, `apply`, `save`,
`discard`, `snapshots`, `undo`, `export`, `import`, `smart`, `automation`, `watch`,
`online` and `accounts`. `depiled accounts --use <name>` picks the account the commands
work on; `--use auto` follows Steam again.

## Limits worth knowing

- Saving needs Steam closed for a moment; Depiled handles it, but a game you're playing will
  be closed with Steam.
- Achievement progress from the internet needs a public profile. Without one, only progress
  Steam has saved on this computer is known.
- The installers aren't code-signed yet, hence the first-launch warnings.
- Windows only for now. Mac builds come back once they've been tested on a Mac.
