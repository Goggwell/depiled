use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub const STEAM64_BASE: u64 = 76_561_197_960_265_728;

pub struct SteamInstall {
    pub dir: PathBuf,
    pub account_id: u32,
}

#[cfg(windows)]
fn steam_dir() -> Result<PathBuf> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Valve\Steam")
        .context("Steam registry key not found")?;
    let dir: String = key.get_value("SteamPath")?;
    Ok(PathBuf::from(dir))
}

#[cfg(not(windows))]
fn steam_dir() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from).context("HOME is not set")?;
    let candidates = if cfg!(target_os = "macos") {
        vec![home.join("Library/Application Support/Steam")]
    } else {
        vec![home.join(".steam/steam"), home.join(".local/share/Steam")]
    };
    candidates.into_iter().find(|d| d.join("userdata").is_dir()).context("Steam folder not found")
}

/// A Steam account that has been used on this computer.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Account {
    pub id: u32,
    pub steam64: u64,
    /// The name friends see, if Steam remembers it.
    pub name: String,
    /// The name used to sign in, if Steam remembers it.
    pub login: String,
    /// When it last signed in to Steam here (Unix seconds), 0 if unknown.
    pub last_login: u64,
    /// When Steam last wrote to its folder, as a fallback for ordering.
    #[serde(skip)]
    pub touched: u64,
    /// Steam's cached copy of its profile picture.
    pub avatar: Option<PathBuf>,
}

/// Every account with a library on this computer: a folder under `userdata` holding Steam's
/// list of its licenses. (Folders without one belong to accounts that only ever signed in
/// briefly, and have nothing to organize.) Names come from Steam's list of remembered logins.
pub fn accounts(dir: &Path) -> Vec<Account> {
    let logins = crate::vdf::read_text(&dir.join("config").join("loginusers.vdf")).ok().and_then(|t| crate::vdf::parse_text(&t).ok());
    let users = logins.as_ref().and_then(|v| v.get("users"));
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir.join("userdata")).into_iter().flatten().flatten() {
        let Ok(id) = entry.file_name().to_string_lossy().parse::<u32>() else { continue };
        if id == 0 || !entry.path().join("config").join("licensecache").is_file() {
            continue;
        }
        let steam64 = STEAM64_BASE + u64::from(id);
        let user = users.and_then(|u| u.get(&steam64.to_string()));
        let text = |key: &str| user.and_then(|u| u.get(key)).and_then(crate::vdf::Value::as_str).unwrap_or_default().to_owned();
        let touched = entry.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
        let avatar = dir.join("config").join("avatarcache").join(format!("{steam64}.png"));
        out.push(Account {
            id,
            steam64,
            name: text("PersonaName"),
            login: text("AccountName"),
            last_login: text("timestamp").parse().unwrap_or(0),
            touched,
            avatar: avatar.is_file().then_some(avatar),
        });
    }
    // Most recently used first.
    out.sort_by_key(|a| std::cmp::Reverse((a.last_login, a.touched)));
    out
}

/// The account signed in to Steam right now, if Steam is running and someone is.
pub fn signed_in(dir: &Path) -> Option<u32> {
    #[cfg(windows)]
    {
        let _ = dir;
        use winreg::RegKey;
        use winreg::enums::HKEY_CURRENT_USER;
        let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\Valve\Steam\ActiveProcess").ok()?;
        let id: u32 = key.get_value("ActiveUser").ok()?;
        (id != 0).then_some(id)
    }
    #[cfg(not(windows))]
    {
        let text = std::fs::read_to_string(dir.join("registry.vdf")).ok()?;
        Some(parse_registry_number(&text, "ActiveUser")).filter(|&id| id != 0)
    }
}

/// Which account Depiled shows: the one the user picked, if it's still here; otherwise
/// whoever is signed in to Steam; otherwise whoever signed in last.
pub fn choose(accounts: &[Account], picked: Option<u32>, signed_in: Option<u32>) -> Option<u32> {
    let here = |id: u32| accounts.iter().any(|a| a.id == id);
    picked.filter(|&id| here(id)).or(signed_in.filter(|&id| here(id))).or_else(|| accounts.first().map(|a| a.id))
}

impl SteamInstall {
    /// Locates Steam and the account to show (see `choose`).
    pub fn detect() -> Result<Self> {
        let dir = steam_dir()?;
        let picked = crate::settings::load().ok().and_then(|s| s.account);
        let accounts = accounts(&dir);
        let account_id = choose(&accounts, picked, signed_in(&dir)).context("no Steam account folders found")?;
        Ok(Self { dir, account_id })
    }

    pub fn steam64(&self) -> u64 {
        STEAM64_BASE + u64::from(self.account_id)
    }

    pub fn config_dir(&self) -> PathBuf {
        self.dir
            .join("userdata")
            .join(self.account_id.to_string())
            .join("config")
    }

    pub fn cloudstorage_dir(&self) -> PathBuf {
        self.config_dir().join("cloudstorage")
    }

    /// The game currently running, as reported by the Steam client (0 means none).
    #[cfg(windows)]
    pub fn running_app_id(&self) -> Result<u32> {
        use winreg::RegKey;
        use winreg::enums::HKEY_CURRENT_USER;
        let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\Valve\Steam")?;
        Ok(key.get_value("RunningAppID")?)
    }

    /// The game currently running, as reported by the Steam client (0 means none).
    #[cfg(not(windows))]
    pub fn running_app_id(&self) -> Result<u32> {
        let text = std::fs::read_to_string(self.dir.join("registry.vdf")).context("reading registry.vdf")?;
        Ok(parse_registry_number(&text, "RunningAppID"))
    }
}

/// Pulls a number such as `"RunningAppID"  "123"` out of registry.vdf.
#[cfg_attr(windows, allow(dead_code))]
fn parse_registry_number(text: &str, key: &str) -> u32 {
    let quoted = format!("\"{key}\"");
    text.lines()
        .find(|l| l.contains(&quoted))
        .and_then(|l| l.split('"').nth(3))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

pub fn steam_is_running() -> Result<bool> {
    use sysinfo::{ProcessesToUpdate, System};
    let name = if cfg!(windows) {
        "steam.exe"
    } else if cfg!(target_os = "macos") {
        "steam_osx"
    } else {
        "steam"
    };
    let mut sys = System::new()?;
    sys.refresh_processes(ProcessesToUpdate::All, true);
    Ok(sys.processes_by_exact_name(name.as_ref()).next().is_some())
}
/// The two-letter country this computer is set to, used as the default store region.
pub fn home_region() -> Option<String> {
    #[cfg(windows)]
    {
        use winreg::RegKey;
        use winreg::enums::HKEY_CURRENT_USER;
        let name: Option<String> = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(r"Control Panel\International\Geo")
            .ok()
            .and_then(|k| k.get_value("Name").ok());
        if let Some(n) = name.filter(|n| n.len() == 2) {
            return Some(n.to_ascii_uppercase());
        }
    }
    // macOS and Linux: the region part of the locale, e.g. en_GB.UTF-8.
    ["LC_ALL", "LC_MONETARY", "LANG"].iter().filter_map(|v| std::env::var(v).ok()).find_map(|l| {
        let lang = l.split(['.', '@']).next()?;
        lang.split('_').nth(1).filter(|r| r.len() == 2).map(str::to_ascii_uppercase)
    })
}

/// Asks a running Steam to exit, the same way its own menu does.
pub fn shutdown() -> Result<()> {
    let install = SteamInstall::detect()?;
    if cfg!(windows) {
        std::process::Command::new(install.dir.join("steam.exe")).arg("-shutdown").spawn()?;
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("osascript").args(["-e", "quit app \"Steam\""]).spawn()?;
    } else {
        std::process::Command::new("steam").arg("-shutdown").spawn()?;
    }
    Ok(())
}

pub fn start(install: &SteamInstall) -> Result<()> {
    if cfg!(windows) {
        std::process::Command::new(install.dir.join("steam.exe")).spawn()?;
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").args(["-a", "Steam"]).spawn()?;
    } else {
        std::process::Command::new("steam").spawn()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_numbers_from_the_registry_file() {
        let vdf = "\"Registry\"\n{\n\t\"Steam\"\n\t{\n\t\t\"RunningAppID\"\t\t\"620\"\n\t\t\"ActiveProcess\"\n\t\t{\n\t\t\t\"ActiveUser\"\t\t\"42\"\n\t\t}\n\t}\n}\n";
        assert_eq!(parse_registry_number(vdf, "RunningAppID"), 620);
        assert_eq!(parse_registry_number(vdf, "ActiveUser"), 42);
        assert_eq!(parse_registry_number("nothing here", "RunningAppID"), 0);
    }

    /// A Steam folder with three accounts: two with libraries and names, one that never got a
    /// library, plus Steam's anonymous folder.
    pub(crate) fn fake_steam(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("depiled-steam-{name}-{}", crate::snapshot::now_secs()));
        for (id, library) in [(11, true), (22, true), (33, false), (0, true)] {
            let config = dir.join("userdata").join(id.to_string()).join("config");
            std::fs::create_dir_all(&config).unwrap();
            if library {
                std::fs::write(config.join("licensecache"), b"").unwrap();
            }
        }
        let steam64 = |id: u64| STEAM64_BASE + id;
        std::fs::create_dir_all(dir.join("config").join("avatarcache")).unwrap();
        std::fs::write(dir.join("config").join("avatarcache").join(format!("{}.png", steam64(22))), b"png").unwrap();
        std::fs::write(
            dir.join("config").join("loginusers.vdf"),
            format!(
                "\"users\"\n{{\n\t\"{}\"\n\t{{\n\t\t\"AccountName\"\t\t\"main_login\"\n\t\t\"PersonaName\"\t\t\"Main\"\n\t\t\"timestamp\"\t\t\"2000\"\n\t}}\n\t\"{}\"\n\t{{\n\t\t\"AccountName\"\t\t\"alt_login\"\n\t\t\"PersonaName\"\t\t\"Alt\"\n\t\t\"timestamp\"\t\t\"1000\"\n\t}}\n}}\n",
                steam64(11),
                steam64(22)
            ),
        )
        .unwrap();
        dir
    }

    #[test]
    fn lists_accounts_with_libraries_newest_sign_in_first() {
        let dir = fake_steam("list");
        let found = accounts(&dir);
        let summary: Vec<(u32, &str, &str, bool)> = found.iter().map(|a| (a.id, a.name.as_str(), a.login.as_str(), a.avatar.is_some())).collect();
        assert_eq!(summary, vec![(11, "Main", "main_login", false), (22, "Alt", "alt_login", true)]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn older_files_go_to_the_account_whose_collections_they_mention() {
        let dir = fake_steam("legacy");
        for (id, collection) in [(11, "uc-MAIN"), (22, "uc-ALT")] {
            let cloud = dir.join("userdata").join(id.to_string()).join("config").join("cloudstorage");
            std::fs::create_dir_all(&cloud).unwrap();
            let value = format!("{{\"id\":\"{collection}\",\"name\":\"X\",\"added\":[1]}}");
            let entries = serde_json::json!([[format!("user-collections.{collection}"), { "key": format!("user-collections.{collection}"), "timestamp": 1, "value": value, "version": "1" }]]);
            std::fs::write(cloud.join("cloud-storage-namespace-1.json"), entries.to_string()).unwrap();
        }
        let app = dir.join("app");
        std::fs::create_dir_all(&app).unwrap();
        // Colours for the second account's collection, though the first signed in last.
        std::fs::write(app.join("colors.json"), r##"{"uc-ALT":"#4a8"}"##).unwrap();
        assert_eq!(crate::config::owner_of_legacy(&app, &dir, &accounts(&dir)), Some(22));
        // Nothing recognisable: the account Steam wrote to last, as older versions chose.
        std::fs::write(app.join("colors.json"), r##"{"uc-GONE":"#4a8"}"##).unwrap();
        assert!(crate::config::owner_of_legacy(&app, &dir, &accounts(&dir)).is_some());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_picked_account_wins_then_the_signed_in_one_then_the_latest() {
        let dir = fake_steam("choose");
        let found = accounts(&dir);
        assert_eq!(choose(&found, Some(22), Some(11)), Some(22), "picked");
        assert_eq!(choose(&found, None, Some(22)), Some(22), "signed in");
        assert_eq!(choose(&found, None, None), Some(11), "latest sign-in");
        assert_eq!(choose(&found, Some(99), None), Some(11), "a picked account that's gone is ignored");
        assert_eq!(choose(&found, None, Some(33)), Some(11), "signed in, but no library yet");
        assert_eq!(choose(&[], None, None), None);
        std::fs::remove_dir_all(dir).unwrap();
    }
}