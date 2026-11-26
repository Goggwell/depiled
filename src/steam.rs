use std::path::PathBuf;

use anyhow::{Context, Result};

const STEAM64_BASE: u64 = 76_561_197_960_265_728;

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

impl SteamInstall {
    /// Locates Steam and picks the most recently active account folder.
    pub fn detect() -> Result<Self> {
        let dir = steam_dir()?;

        let userdata = dir.join("userdata");
        let mut best: Option<(std::time::SystemTime, u32)> = None;
        for entry in std::fs::read_dir(&userdata).context("no userdata folder")? {
            let entry = entry?;
            let Ok(id) = entry.file_name().to_string_lossy().parse::<u32>() else {
                continue;
            };
            let modified = entry.metadata()?.modified()?;
            if best.is_none_or(|(t, _)| modified > t) {
                best = Some((modified, id));
            }
        }
        let (_, account_id) = best.context("no Steam account folders found")?;
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
        Ok(parse_running_app_id(&text))
    }
}

/// Pulls `"RunningAppID"  "123"` out of registry.vdf.
#[cfg_attr(windows, allow(dead_code))]
fn parse_running_app_id(text: &str) -> u32 {
    text.lines()
        .find(|l| l.contains("\"RunningAppID\""))
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
    use super::parse_running_app_id;

    #[test]
    fn reads_running_app_id() {
        let vdf = "\"Registry\"\n{\n\t\"Steam\"\n\t{\n\t\t\"RunningAppID\"\t\t\"620\"\n\t}\n}\n";
        assert_eq!(parse_running_app_id(vdf), 620);
        assert_eq!(parse_running_app_id("nothing here"), 0);
    }
}