use std::env;
use std::fs;

use super::known;

#[derive(Debug, Clone)]
pub struct EnvInfo {
    pub os_pretty: String,
    pub os_kind: String,
    pub pkg_rule: String,
    pub clipboard_line: String,
    pub clipboard_tools: String,
    pub system_tools: String,
    pub disk_usage_hint: String,
    pub known_tools: String,
}

const LINUX_SYSTEM_TOOLS: &str = "systemctl, journalctl, htop, lsof, killall, pstree, fuser, lsusb";
const MACOS_SYSTEM_TOOLS: &str =
    "launchctl, log show, log stream, lsof, killall, pgrep, pkill, system_profiler, pmset, top";
const BSD_SYSTEM_TOOLS: &str = "service, lsof, killall, pgrep, pkill, top";
const UNKNOWN_SYSTEM_TOOLS: &str = "lsof, killall, pgrep, pkill, top";

const GNU_DISK_USAGE_HINT: &str = "For terse disk-usage requests, infer common paths literally: \"tmp\" means /tmp, not the current directory. Prefer one-level summaries such as `du -x --si -d1 PATH 2>/dev/null | sort -hr | head -20`; they are bounded, readable, and still show useful smaller entries. Do not use `du -sh . | grep ...`: it can scan a huge tree silently and then print nothing.\n- If the user says GB/gb in a disk-usage request, prefer decimal SI output (`du --si`) over filtering only lines with a G suffix, unless they explicitly ask for only GB-sized entries.";
const BSD_DISK_USAGE_HINT: &str = "For terse disk-usage requests, infer common paths literally: \"tmp\" means /tmp, not the current directory. Prefer one-level summaries such as `du -h -x -d1 PATH 2>/dev/null | sort -hr | head -20`; they are bounded, readable, and still show useful smaller entries. BSD `du` has no `--si` — use `-h` (binary units) instead. Do not use `du -sh . | grep ...`: it can scan a huge tree silently and then print nothing.\n- If the user says GB/gb in a disk-usage request, prefer human-readable output (`du -h`) over filtering only lines with a G suffix, unless they explicitly ask for only GB-sized entries.";

fn has_cmd(name: &str) -> bool {
    which::which(name).is_ok()
}

fn read_os_release() -> (String, String, String) {
    // (distro_id, distro_like, pretty_name)
    let mut id = String::new();
    let mut id_like = String::new();
    let mut pretty = String::new();
    if let Ok(s) = fs::read_to_string("/etc/os-release") {
        for line in s.lines() {
            let Some(eq) = line.find('=') else { continue };
            let key = &line[..eq];
            let mut val = &line[eq + 1..];
            if val.starts_with('"') && val.ends_with('"') && val.len() >= 2 {
                val = &val[1..val.len() - 1];
            }
            match key {
                "ID" => id = val.to_string(),
                "ID_LIKE" => id_like = val.to_string(),
                "PRETTY_NAME" => pretty = val.to_string(),
                _ => {}
            }
        }
    }
    (id, id_like, pretty)
}

fn linux_pkg_rule(id_chain: &str) -> &'static str {
    let c = id_chain.to_lowercase();
    if c.contains("nixos") {
        "This is NixOS — never suggest apt, brew, dnf, pacman, or pip-install steps. NixOS is declarative; if a tool may be missing, use a guaranteed alternative."
    } else if c.contains("ubuntu")
        || c.contains("debian")
        || c.contains("mint")
        || c.contains("pop")
        || c.contains("kali")
        || c.contains("raspbian")
        || c.contains("elementary")
    {
        "This is a Debian/Ubuntu-family distro — when a tool is genuinely missing, the install command is 'sudo apt install <pkg>'. Prefer guaranteed-available POSIX tools when possible; only suggest installs if the user explicitly asks."
    } else if c.contains("fedora")
        || c.contains("rhel")
        || c.contains("centos")
        || c.contains("rocky")
        || c.contains("alma")
        || c.contains("amzn")
    {
        "This is a Fedora/RHEL-family distro — when a tool is genuinely missing, the install command is 'sudo dnf install <pkg>'. Prefer guaranteed-available POSIX tools when possible; only suggest installs if the user explicitly asks."
    } else if c.contains("arch")
        || c.contains("manjaro")
        || c.contains("endeavour")
        || c.contains("garuda")
        || c.contains("artix")
        || c.contains("cachyos")
    {
        "This is an Arch-family distro — when a tool is genuinely missing, the install command is 'sudo pacman -S <pkg>'. Prefer guaranteed-available POSIX tools when possible; only suggest installs if the user explicitly asks."
    } else if c.contains("alpine") {
        "This is Alpine — when a tool is genuinely missing, the install command is 'sudo apk add <pkg>'. Note: Alpine uses BusyBox userland, so some GNU-specific flags are unavailable; prefer POSIX-portable ones."
    } else if c.contains("suse") || c.contains("sles") {
        "This is openSUSE/SLES — when a tool is genuinely missing, the install command is 'sudo zypper install <pkg>'. Prefer guaranteed-available POSIX tools when possible."
    } else if c.contains("void") {
        "This is Void Linux — when a tool is genuinely missing, the install command is 'sudo xbps-install -S <pkg>'."
    } else if c.contains("gentoo") {
        "This is Gentoo — install commands ('sudo emerge <pkg>') trigger slow source builds, so avoid suggesting them unless explicitly asked. Prefer guaranteed-available POSIX tools."
    } else {
        "Unknown Linux distribution — don't guess at package-manager commands. If a tool may be missing, use a guaranteed-available POSIX alternative instead of suggesting an install step."
    }
}

fn clipboard_for_unix() -> (String, String) {
    let wayland = env::var("WAYLAND_DISPLAY")
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    let x11 = env::var("DISPLAY").map(|v| !v.is_empty()).unwrap_or(false);
    if wayland && has_cmd("wl-copy") {
        (
            "Wayland clipboard — use 'wl-copy' to copy, 'wl-paste' to paste; never xclip or xsel."
                .into(),
            "wl-copy, wl-paste (Wayland)".into(),
        )
    } else if x11 && has_cmd("xclip") {
        (
            "X11 clipboard — copy with 'xclip -selection clipboard', paste with 'xclip -selection clipboard -o'; never wl-copy.".into(),
            "xclip (X11)".into(),
        )
    } else if x11 && has_cmd("xsel") {
        (
            "X11 clipboard — copy with 'xsel --clipboard --input', paste with 'xsel --clipboard --output'; never wl-copy.".into(),
            "xsel (X11)".into(),
        )
    } else {
        (
            "No clipboard tool detected (headless or no display server) — avoid clipboard commands unless the user explicitly names a tool.".into(),
            "none available".into(),
        )
    }
}

pub fn detect() -> EnvInfo {
    let ostype = std::env::consts::OS;
    let mut info = detect_os(ostype);
    info.known_tools = format_known_tools(&known::load_or_refresh());
    info
}

fn format_known_tools(list: &[String]) -> String {
    let grouped = known::categorize(list);
    if grouped.is_empty() {
        return "(none detected)".to_string();
    }
    let mut out = String::new();
    for (label, tools) in &grouped {
        out.push_str("\n    - ");
        out.push_str(label);
        out.push_str(": ");
        out.push_str(&tools.join(", "));
    }
    out
}

fn detect_os(ostype: &str) -> EnvInfo {
    match ostype {
        "macos" => {
            let pkg_rule = if has_cmd("brew") {
                "On macOS with Homebrew: suggest 'brew install <pkg>' only if the user explicitly asks to install something. macOS ships BSD-userland tools (find, grep, sed, awk) — they differ from GNU; use POSIX-portable flags."
            } else {
                "On macOS without Homebrew: avoid install steps. Use BSD-userland tools that ship with the OS (find, grep, sed, awk) with POSIX-portable flags — they differ from GNU."
            };
            EnvInfo {
                os_pretty: "macOS".into(),
                os_kind: "Darwin".into(),
                pkg_rule: pkg_rule.into(),
                clipboard_line: "macOS pasteboard — use 'pbcopy' to copy, 'pbpaste' to paste.".into(),
                clipboard_tools: "pbcopy, pbpaste (macOS)".into(),
                system_tools: MACOS_SYSTEM_TOOLS.into(),
                disk_usage_hint: BSD_DISK_USAGE_HINT.into(),
                known_tools: String::new(),
            }
        }
        "linux" => {
            let (id, id_like, pretty) = read_os_release();
            let os_pretty = if pretty.is_empty() { "Linux".to_string() } else { pretty };
            let id_chain = format!("{} {}", id, id_like);
            let pkg_rule = linux_pkg_rule(&id_chain);
            let (clipboard_line, clipboard_tools) = clipboard_for_unix();
            EnvInfo {
                os_pretty,
                os_kind: "Linux".into(),
                pkg_rule: pkg_rule.into(),
                clipboard_line,
                clipboard_tools,
                system_tools: LINUX_SYSTEM_TOOLS.into(),
                disk_usage_hint: GNU_DISK_USAGE_HINT.into(),
                known_tools: String::new(),
            }
        }
        "freebsd" | "openbsd" | "netbsd" | "dragonfly" => {
            let (clipboard_line, clipboard_tools) = clipboard_for_unix();
            EnvInfo {
                os_pretty: ostype.to_string(),
                os_kind: "BSD".into(),
                pkg_rule: "This is BSD — when a tool is genuinely missing, the install command is 'sudo pkg install <pkg>'. BSD userland differs from GNU; use POSIX-portable flags.".into(),
                clipboard_line,
                clipboard_tools,
                system_tools: BSD_SYSTEM_TOOLS.into(),
                disk_usage_hint: BSD_DISK_USAGE_HINT.into(),
                known_tools: String::new(),
            }
        }
        other => EnvInfo {
            os_pretty: other.to_string(),
            os_kind: "Unknown".into(),
            pkg_rule: "Unknown OS — don't suggest install commands; pick guaranteed-available POSIX tools.".into(),
            clipboard_line: "No clipboard tool assumed — avoid clipboard commands unless the user explicitly names a tool.".into(),
            clipboard_tools: "none assumed".into(),
            system_tools: UNKNOWN_SYSTEM_TOOLS.into(),
            disk_usage_hint: BSD_DISK_USAGE_HINT.into(),
            known_tools: String::new(),
        },
    }
}
