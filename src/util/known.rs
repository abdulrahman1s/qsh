use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::cache;

const TTL: Duration = Duration::from_secs(60 * 60 * 24 * 7);

pub const CATEGORIES: &[(&str, &[&str])] = &[
    (
        "search/files",
        &[
            "rg", "rga", "fd", "ag", "ack", "fzf", "bat", "eza", "exa", "lsd", "tree", "file",
            "trash", "dust", "ncdu", "gdu", "duf", "broot", "nnn", "ranger", "lf", "yazi", "sd",
            "choose",
        ],
    ),
    (
        "archives",
        &[
            "7z", "unrar", "zip", "unzip", "xz", "zstd", "lz4", "lzip", "lzop", "brotli", "ouch",
        ],
    ),
    (
        "network",
        &[
            "curl",
            "wget",
            "aria2c",
            "httpie",
            "http",
            "xh",
            "dig",
            "drill",
            "host",
            "nslookup",
            "whois",
            "dog",
            "q",
            "nc",
            "ncat",
            "socat",
            "rsync",
            "ssh",
            "sftp",
            "scp",
            "mosh",
            "nmap",
            "tcpdump",
            "ngrep",
            "mtr",
            "traceroute",
            "iperf",
            "iperf3",
            "speedtest-cli",
            "gping",
            "cloudflared",
            "tailscale",
            "wg",
        ],
    ),
    (
        "dev tools",
        &[
            "git",
            "gh",
            "glab",
            "jq",
            "yq",
            "gum",
            "delta",
            "tig",
            "lazygit",
            "direnv",
            "mise",
            "asdf",
            "devbox",
            "watchexec",
            "entr",
            "hyperfine",
            "tokei",
            "scc",
            "shellcheck",
            "shfmt",
            "hadolint",
            "pre-commit",
            "trivy",
            "grype",
            "syft",
            "cosign",
            "ast-grep",
            "sg",
            "semgrep",
        ],
    ),
    (
        "containers/orchestration",
        &[
            "docker",
            "docker-compose",
            "podman",
            "podman-compose",
            "nerdctl",
            "kubectl",
            "helm",
            "helmfile",
            "kustomize",
            "k9s",
            "k3s",
            "k3d",
            "kind",
            "minikube",
            "kubectx",
            "kubens",
            "stern",
            "skaffold",
            "argocd",
            "flux",
            "buildah",
        ],
    ),
    (
        "cloud/infra",
        &[
            "terraform",
            "tofu",
            "pulumi",
            "ansible",
            "aws",
            "gcloud",
            "az",
            "doctl",
            "flyctl",
            "fly",
            "heroku",
            "vercel",
            "railway",
            "vault",
            "packer",
            "consul",
            "nomad",
            "eksctl",
        ],
    ),
    (
        "languages/pkg managers",
        &[
            "node", "deno", "bun", "npm", "pnpm", "yarn", "npx", "tsc", "ts-node", "python",
            "python3", "pip", "pipx", "poetry", "uv", "pdm", "rye", "conda", "mamba", "ruby",
            "gem", "bundle", "go", "rustc", "cargo", "rustup", "java", "mvn", "gradle", "kotlin",
            "scala", "sbt", "php", "composer", "elixir", "mix", "iex", "dart", "flutter", "swift",
            "dotnet", "lua", "luarocks", "perl", "cpan", "julia", "R", "ghc", "cabal", "stack",
            "ocaml", "opam", "zig", "crystal", "nim", "gleam", "v",
        ],
    ),
    (
        "build/task",
        &[
            "make", "just", "ninja", "cmake", "meson", "bazel", "buck", "buck2", "task", "mage",
            "earthly",
        ],
    ),
    (
        "system",
        &[
            "htop",
            "btop",
            "btm",
            "iotop",
            "iftop",
            "nethogs",
            "glances",
            "atop",
            "neofetch",
            "fastfetch",
            "pstree",
            "fuser",
            "lsusb",
            "lsblk",
            "lsof",
            "procs",
            "smartctl",
            "sensors",
            "dmidecode",
            "free",
            "vmstat",
            "uptime",
            "pmap",
            "strace",
            "ltrace",
        ],
    ),
    (
        "media",
        &[
            "ffmpeg",
            "ffprobe",
            "yt-dlp",
            "youtube-dl",
            "mpv",
            "vlc",
            "sox",
            "imagemagick",
            "magick",
            "convert",
            "exiftool",
            "gifsicle",
            "oxipng",
            "jpegoptim",
            "pngquant",
            "ghostscript",
            "gs",
            "pandoc",
            "asciinema",
            "agg",
            "vhs",
            "freeze",
            "glow",
            "mdcat",
        ],
    ),
    (
        "editors/multiplexers",
        &[
            "vi", "vim", "nano", "nvim", "emacs", "code", "helix", "hx", "micro", "kak", "ed",
            "subl", "tmux", "screen", "zellij",
        ],
    ),
    (
        "shells",
        &[
            "bash", "zsh", "fish", "dash", "ksh", "mksh", "ash", "elvish", "nu", "xonsh", "osh",
        ],
    ),
    (
        "security/crypto",
        &[
            "gpg",
            "gpg2",
            "age",
            "sops",
            "pass",
            "bw",
            "openssl",
            "ssh-keygen",
            "ssh-agent",
            "ssh-add",
            "minisign",
            "signify",
        ],
    ),
    (
        "databases",
        &[
            "psql",
            "pg_dump",
            "pg_restore",
            "mysql",
            "mysqldump",
            "mariadb",
            "sqlite3",
            "redis-cli",
            "valkey-cli",
            "mongosh",
            "mongo",
            "duckdb",
            "clickhouse-client",
            "influx",
        ],
    ),
    ("version control (non-git)", &["hg", "svn", "fossil", "jj"]),
    (
        "shell qol",
        &[
            "starship", "zoxide", "atuin", "mcfly", "navi", "thefuck", "tldr", "tealdeer", "cheat",
            "eg",
        ],
    ),
    (
        "package managers",
        &[
            "brew",
            "apt",
            "apt-get",
            "dnf",
            "yum",
            "pacman",
            "yay",
            "paru",
            "apk",
            "zypper",
            "xbps-install",
            "emerge",
            "nix",
            "snap",
            "flatpak",
            "port",
            "pkg",
        ],
    ),
    (
        "clipboard",
        &["wl-copy", "wl-paste", "xclip", "xsel", "pbcopy", "pbpaste"],
    ),
];

pub fn known_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join(".qsh_known")
}

pub fn load_or_refresh() -> Vec<String> {
    let path = known_path();
    if is_fresh(&path)
        && let Some(list) = read_list(&path)
    {
        return list;
    }
    refresh()
}

pub fn refresh() -> Vec<String> {
    let list: Vec<String> = CATEGORIES
        .iter()
        .flat_map(|(_, tools)| tools.iter())
        .filter(|name| which::which(name).is_ok())
        .map(|s| s.to_string())
        .collect();
    let _ = write_list(&known_path(), &list);
    list
}

/// Group an installed-tool list into category buckets, preserving the
/// CATEGORIES order. User-added entries that aren't in any category go
/// into a trailing "other" bucket.
pub fn categorize(installed: &[String]) -> Vec<(&'static str, Vec<String>)> {
    use std::collections::HashSet;
    let installed_set: HashSet<&str> = installed.iter().map(String::as_str).collect();
    let mut placed: HashSet<&str> = HashSet::new();
    let mut out: Vec<(&'static str, Vec<String>)> = Vec::new();
    for (label, tools) in CATEGORIES {
        let bucket: Vec<String> = tools
            .iter()
            .filter(|t| installed_set.contains(*t))
            .map(|t| {
                placed.insert(t);
                (*t).to_string()
            })
            .collect();
        if !bucket.is_empty() {
            out.push((label, bucket));
        }
    }
    let other: Vec<String> = installed
        .iter()
        .filter(|t| !placed.contains(t.as_str()))
        .cloned()
        .collect();
    if !other.is_empty() {
        out.push(("other", other));
    }
    out
}

fn is_fresh(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    SystemTime::now()
        .duration_since(modified)
        .map(|age| age < TTL)
        .unwrap_or(false)
}

fn read_list(path: &Path) -> Option<Vec<String>> {
    let s = fs::read_to_string(path).ok()?;
    let list: Vec<String> = s
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect();
    if list.is_empty() { None } else { Some(list) }
}

fn write_list(path: &Path, list: &[String]) -> std::io::Result<()> {
    let mut content = String::new();
    content.push_str("# qsh known programs — auto-generated.\n");
    content.push_str("# Edit freely; lines starting with '#' and blank lines are ignored.\n");
    content.push_str("# Refresh manually: qsh known --refresh\n\n");
    for name in list {
        content.push_str(name);
        content.push('\n');
    }
    cache::save_atomic(path, content.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_list_skips_comments_and_blanks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("known");
        fs::write(&path, "# header\n\nrg\n  fd  \n# another comment\njq\n").unwrap();
        let list = read_list(&path).unwrap();
        assert_eq!(list, vec!["rg", "fd", "jq"]);
    }

    #[test]
    fn read_list_empty_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("known");
        fs::write(&path, "# only comments\n\n").unwrap();
        assert!(read_list(&path).is_none());
    }

    #[test]
    fn write_list_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("known");
        let list = vec!["rg".to_string(), "fd".to_string(), "jq".to_string()];
        write_list(&path, &list).unwrap();
        let read = read_list(&path).unwrap();
        assert_eq!(read, list);
    }

    #[test]
    fn is_fresh_false_for_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_fresh(&dir.path().join("nope")));
    }

    #[test]
    fn categorize_groups_in_category_order() {
        let installed: Vec<String> = ["jq", "rg", "ffmpeg", "fd", "wget"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let out = categorize(&installed);
        let labels: Vec<&str> = out.iter().map(|(l, _)| *l).collect();
        assert_eq!(
            labels,
            vec!["search/files", "network", "dev tools", "media"]
        );
        assert_eq!(out[0].1, vec!["rg", "fd"]);
        assert_eq!(out[1].1, vec!["wget"]);
        assert_eq!(out[2].1, vec!["jq"]);
        assert_eq!(out[3].1, vec!["ffmpeg"]);
    }

    #[test]
    fn categorize_puts_unknown_into_other() {
        let installed: Vec<String> = ["rg", "my-custom-tool", "another"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let out = categorize(&installed);
        let last = out.last().unwrap();
        assert_eq!(last.0, "other");
        assert_eq!(last.1, vec!["my-custom-tool", "another"]);
    }

    #[test]
    fn categorize_empty_input_empty_output() {
        assert!(categorize(&[]).is_empty());
    }
}
