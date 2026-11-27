//! Bug reports, feedback and crash logs.
//!
//! Nothing is sent unless the user presses Send. A report carries what they wrote and, if they
//! leave it ticked, a short technical summary: versions, counts and recent error messages. It
//! never includes the Steam account, the library's contents or anything from Steam's files,
//! and home folders and Steam ids are blanked out of every message before it's shown or sent.
//!
//! Reports go to a form service that forwards them by email. Its address is added when the
//! release is built (the `DEPILED_FEEDBACK_URL` build variable), so the developer's email isn't
//! in the code. A build without it can still copy a report to paste elsewhere.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::config;

/// Where reports are sent, if this build was given one.
pub const ENDPOINT: Option<&str> = option_env!("DEPILED_FEEDBACK_URL");

/// Longest report text sent, so a huge log can't be rejected or clog an inbox.
const MAX_TEXT: usize = 60_000;

// ---- privacy ----------------------------------------------------------------------------------

/// Blanks out what could identify the user: their home folder (which holds their Windows user
/// name) and Steam ids, wherever they appear in a message or log.
pub fn scrub(text: &str) -> String {
    let mut out = text.to_owned();
    let homes = ["USERPROFILE", "HOME"].iter().filter_map(std::env::var_os).map(|h| h.to_string_lossy().into_owned()).filter(|h| h.len() > 3);
    for home in homes {
        for form in [home.clone(), home.replace('\\', "/"), home.replace('\\', "\\\\")] {
            out = replace_ignore_case(&out, &form, "~");
        }
    }
    if let Ok(user) = std::env::var("USERNAME").or_else(|_| std::env::var("USER"))
        && user.len() > 2
    {
        out = replace_ignore_case(&out, &user, "<user>");
    }
    blank_steam_ids(&out)
}

fn replace_ignore_case(text: &str, find: &str, with: &str) -> String {
    let lower = text.to_lowercase();
    let needle = find.to_lowercase();
    // Lower-casing can change byte lengths outside ASCII; only then fall back to exact matching.
    if needle.is_empty() || lower.len() != text.len() {
        return text.replace(find, with);
    }
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while let Some(i) = lower[at..].find(&needle) {
        out.push_str(&text[at..at + i]);
        out.push_str(with);
        at += i + needle.len();
    }
    out.push_str(&text[at..]);
    out
}

/// 17-digit Steam ids (7656119…) and the account ids in Steam's folder paths.
fn blank_steam_ids(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        if text[i..].starts_with("7656119") {
            let len = bytes[i..].iter().take_while(|b| b.is_ascii_digit()).count();
            if len == 17 {
                out.push_str("<steam id>");
                i += len;
                continue;
            }
        }
        if let Some(rest) = text[i..].strip_prefix("userdata") {
            let sep = rest.chars().next().filter(|c| *c == '/' || *c == '\\');
            if let Some(sep) = sep {
                let digits = rest[1..].bytes().take_while(u8::is_ascii_digit).count();
                if digits > 0 {
                    out.push_str("userdata");
                    out.push(sep);
                    out.push_str("<account>");
                    i += "userdata".len() + 1 + digits;
                    continue;
                }
            }
        }
        let ch = text[i..].chars().next().unwrap_or(' ');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

// ---- technical details ------------------------------------------------------------------------

/// The operating system in words, e.g. "Windows 11 Home 24H2 (build 26200)".
pub fn os_description() -> String {
    #[cfg(windows)]
    {
        use winreg::RegKey;
        use winreg::enums::HKEY_LOCAL_MACHINE;
        if let Ok(key) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion") {
            let get = |name: &str| key.get_value::<String, _>(name).unwrap_or_default();
            let build = get("CurrentBuild");
            // Windows 11 still reports itself as Windows 10 in ProductName.
            let product = get("ProductName");
            let product = if build.parse::<u32>().is_ok_and(|b| b >= 22000) { product.replacen("Windows 10", "Windows 11", 1) } else { product };
            return format!("{product} {} (build {build}), {}", get("DisplayVersion"), std::env::consts::ARCH).trim().to_owned();
        }
    }
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

/// The technical part of a report, as plain text the user can read before sending.
pub fn details(app_version: &str, summary: &[(String, String)], recent_errors: &[String]) -> String {
    let mut out = format!("Depiled {app_version}\n{}\n", os_description());
    for (k, v) in summary {
        out.push_str(&format!("{k}: {v}\n"));
    }
    if !recent_errors.is_empty() {
        out.push_str("\nRecent errors, newest last:\n");
        for e in recent_errors.iter().rev().take(15).rev() {
            out.push_str(&format!("- {}\n", e.trim()));
        }
    }
    scrub(&out)
}

// ---- crash logs -------------------------------------------------------------------------------

/// "2026-11-27 13:19 UTC".
fn utc_time(secs: u64) -> String {
    let (y, m, d) = crate::online::utc_date(secs);
    format!("{y}-{m:02}-{d:02} {:02}:{:02} UTC", secs % 86_400 / 3600, secs % 3600 / 60)
}

fn crash_dir() -> Result<PathBuf> {
    Ok(config::dir()?.join("crashes"))
}

/// A crash log waiting for the user to send it or not.
#[derive(Debug, Clone, Serialize)]
pub struct Crash {
    pub id: String,
    /// Unix seconds.
    pub at: u64,
    /// The first line: what went wrong, in the program's words.
    pub summary: String,
    pub text: String,
}

/// Writes a crash log when the app panics, then starts `reporter` (the app itself, with a flag)
/// so the user is asked about it straight away, the way games show a crash reporter. With
/// \`panic = "abort"\` every panic ends the program, so this runs for all of them.
pub fn install_crash_handler(app_version: &'static str, reporter: Option<fn(&Path)>) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = info.payload().downcast_ref::<&str>().map(ToString::to_string).or_else(|| info.payload().downcast_ref::<String>().cloned()).unwrap_or_else(|| "unknown panic".to_owned());
        let place = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        let thread = std::thread::current().name().unwrap_or("unnamed").to_owned();
        let backtrace = std::backtrace::Backtrace::force_capture();
        let at = crate::snapshot::now_secs();
        let text = scrub(&format!(
            "Depiled {app_version} stopped: {message}\nat {place} (thread {thread})\n{}\nTime: {}\n\nBacktrace:\n{backtrace}\n",
            os_description(),
            utc_time(at)
        ));
        if let Ok(dir) = crash_dir()
            && std::fs::create_dir_all(&dir).is_ok()
        {
            let file = dir.join(format!("crash-{at}.txt"));
            if std::fs::write(&file, &text).is_ok()
                && let Some(reporter) = reporter
            {
                reporter(&file);
            }
        }
        previous(info);
    }));
}

/// Crash logs the user hasn't sent or dismissed yet, newest first.
pub fn pending_crashes() -> Vec<Crash> {
    let Ok(dir) = crash_dir() else { return Vec::new() };
    let mut out: Vec<Crash> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let at = name.strip_prefix("crash-")?.strip_suffix(".txt")?.parse().ok()?;
            let text = std::fs::read_to_string(e.path()).ok()?;
            let summary = text.lines().next().unwrap_or_default().to_owned();
            Some(Crash { id: name, at, summary, text })
        })
        .collect();
    out.sort_by_key(|c| std::cmp::Reverse(c.at));
    out
}

/// One crash log, by id (its file name).
pub fn crash(id: &str) -> Option<Crash> {
    pending_crashes().into_iter().find(|c| c.id == id)
}

/// Done with a crash log, sent or not. It's moved aside rather than deleted, so a copy stays on
/// this computer; only the last 20 are kept.
pub fn settle_crash(id: &str) -> Result<()> {
    if id.contains(['/', '\\']) || !id.starts_with("crash-") {
        bail!("not a crash log");
    }
    let dir = crash_dir()?;
    let done = dir.join("handled");
    std::fs::create_dir_all(&done)?;
    std::fs::rename(dir.join(id), done.join(id))?;
    let mut old: Vec<PathBuf> = std::fs::read_dir(&done)?.flatten().map(|e| e.path()).collect();
    old.sort();
    for p in old.iter().take(old.len().saturating_sub(20)) {
        let _ = std::fs::remove_file(p);
    }
    Ok(())
}

// ---- sending ----------------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct Report {
    /// "bug", "idea", "other" or "crash".
    pub kind: String,
    /// What the user wrote.
    pub message: String,
    /// Where to reply, if the user wants one.
    #[serde(default)]
    pub email: Option<String>,
    /// The technical part, already shown to the user.
    #[serde(default)]
    pub details: Option<String>,
}

#[derive(Serialize)]
struct Form<'a> {
    #[serde(rename = "_subject")]
    subject: String,
    kind: &'a str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<String>,
}

/// The whole report as one piece of text, for copying when it can't be sent.
pub fn as_text(r: &Report) -> String {
    let mut out = format!("Depiled {} report\n\n{}\n", r.kind, r.message.trim());
    if let Some(e) = r.email.as_deref().filter(|e| !e.trim().is_empty()) {
        out.push_str(&format!("\nReply to: {e}\n"));
    }
    if let Some(d) = &r.details {
        out.push_str(&format!("\n---\n{d}"));
    }
    out
}

pub fn send(r: &Report) -> Result<()> {
    let Some(url) = ENDPOINT.filter(|u| !u.is_empty()) else {
        bail!("this copy of Depiled can't send reports itself, so copy the report instead");
    };
    if r.message.trim().is_empty() && r.kind != "crash" {
        bail!("write something first");
    }
    let email = r.email.as_deref().map(str::trim).filter(|e| !e.is_empty());
    if email.is_some_and(|e| !e.contains('@') || e.contains(char::is_whitespace)) {
        bail!("that email address doesn't look right");
    }
    let first_line = r.message.lines().find(|l| !l.trim().is_empty()).unwrap_or("(no message)");
    let short: String = first_line.chars().take(70).collect();
    let mut details = r.details.clone();
    if let Some(d) = details.as_mut()
        && d.len() > MAX_TEXT
    {
        let cut = (0..=MAX_TEXT).rev().find(|&i| d.is_char_boundary(i)).unwrap_or(0);
        d.truncate(cut);
        d.push_str("\n[cut short]");
    }
    let form = Form { subject: format!("Depiled {}: {short}", r.kind), kind: &r.kind, message: r.message.trim(), email, details };
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .user_agent(crate::online::USER_AGENT)
        .http_status_as_error(false)
        .build()
        .new_agent();
    let mut resp = agent.post(url).header("Accept", "application/json").send_json(&form).context("couldn't reach the report service; check your connection")?;
    let status = resp.status();
    if status.is_success() {
        return Ok(());
    }
    let body = resp.body_mut().read_to_string().unwrap_or_default();
    match status.as_u16() {
        429 => bail!("too many reports were sent recently; try again in a while"),
        _ => bail!("the report service said no ({status}){}", if body.len() < 300 && !body.is_empty() { format!(": {body}") } else { String::new() }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn personal_details_are_blanked_out() {
        let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap();
        let text = format!("reading {home}\\AppData\\x.json and D:/steam/userdata/879673802/config, profile 76561198839939530");
        let clean = scrub(&text);
        assert!(!clean.contains(&home), "{clean}");
        assert!(clean.contains("~"), "{clean}");
        assert!(clean.contains("userdata/<account>/config"), "{clean}");
        assert!(clean.contains("<steam id>") && !clean.contains("76561198839939530"), "{clean}");
        // Other numbers are left alone.
        assert_eq!(blank_steam_ids("app 620, 1234567890123"), "app 620, 1234567890123");
    }

    #[test]
    fn a_report_reads_as_plain_text() {
        let r = Report { kind: "bug".into(), message: " Save failed \n".into(), email: Some("me@example.com".into()), details: Some("Depiled 0.2.2".into()) };
        assert_eq!(as_text(&r), "Depiled bug report\n\nSave failed\n\nReply to: me@example.com\n\n---\nDepiled 0.2.2");
    }

    #[test]
    fn crash_times_read_as_dates() {
        assert_eq!(utc_time(1_795_756_740), "2026-11-27 05:19 UTC");
        assert_eq!(utc_time(0), "1970-01-01 00:00 UTC");
    }

    #[test]
    fn crash_ids_cant_point_outside_the_folder() {
        assert!(settle_crash("../settings.json").is_err());
        assert!(settle_crash("crash-1/../../x").is_err());
    }
}
