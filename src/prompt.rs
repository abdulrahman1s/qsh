use crate::cli::Shell;
use crate::config::Mode;
use crate::env_detect::EnvInfo;

const SYSTEM_PROMPT_TEMPLATE: &str = include_str!("../prompts/system.txt");
const RETRY_DIRECTIVE: &str = include_str!("../prompts/retry.txt");
const REFINE_DIRECTIVE: &str = include_str!("../prompts/refine.txt");
const EXPLAIN_DIRECTIVE: &str = include_str!("../prompts/explain.txt");
const ALTS_DIRECTIVE_TEMPLATE: &str = include_str!("../prompts/alts.txt");

pub fn system_prompt(env: &EnvInfo, shell: Shell) -> String {
    SYSTEM_PROMPT_TEMPLATE
        .replace("__SHELL_NAME__", shell.as_str())
        .replace("__SHELL_DESCRIPTION__", shell_description(shell))
        .replace("__SHELL_SYNTAX__", shell_syntax(shell))
        .replace("__SHELL_EXAMPLES__", shell_examples(shell))
        .replace("__OS_PRETTY__", &env.os_pretty)
        .replace("__OS_KIND__", &env.os_kind)
        .replace("__CLIPBOARD_LINE__", &env.clipboard_line)
        .replace("__CLIPBOARD_TOOLS__", &env.clipboard_tools)
        .replace("__PKG_RULE__", &env.pkg_rule)
}

fn shell_description(shell: Shell) -> &'static str {
    match shell {
        Shell::Zsh => "zsh with EXTENDED_GLOB, NOMATCH, AUTOCD",
        Shell::Bash => "bash with standard Bash semantics",
        Shell::Fish => "fish with native Fish syntax and command substitutions",
    }
}

fn shell_syntax(shell: Shell) -> &'static str {
    match shell {
        Shell::Zsh | Shell::Bash => {
            "- Use POSIX-style shell syntax unless the request clearly needs a shell-specific feature.\n- Command substitution is `$(...)`; variables are `$name` or `${name}`; loops use `for x in ...; do ...; done`.\n- Use `&&` and `||` for conditional chaining."
        }
        Shell::Fish => {
            "- Generate Fish syntax, not POSIX sh/bash syntax.\n- Command substitution is `(...)`, not `$(...)`; variables are `$name`, not `${name}`.\n- Loops and conditionals use Fish blocks: `for x in ...; ...; end` and `if ...; ...; end`. There is no `do`, `then`, or `fi`.\n- Use `set` for variables and `env NAME=value command` for one-shot environment variables.\n- Prefer `and` and `or` for conditional chaining."
        }
    }
}

fn shell_examples(shell: Shell) -> &'static str {
    match shell {
        Shell::Zsh | Shell::Bash => {
            r#"INPUT: find rust files modified in the last week
OUTPUT: find . -type f -name '*.rs' -mtime -7

INPUT: kill whatever is listening on port 3000
OUTPUT: kill -9 "$(lsof -t -i:3000)"

INPUT: 10 largest files under this directory
OUTPUT: du -ah . 2>/dev/null | sort -rh | head -10

INPUT: tmp gb
OUTPUT: du -x --si -d1 /tmp 2>/dev/null | sort -hr | head -20

INPUT: largest folders here
OUTPUT: du -x --si -d1 . 2>/dev/null | sort -hr | head -20

INPUT: pretty-print package.json dependencies
OUTPUT: jq '.dependencies' package.json

INPUT: follow nginx logs
OUTPUT: journalctl -u nginx -b -f --no-hostname

INPUT: extract every .tar.gz here into its own folder
OUTPUT: for f in *.tar.gz; do mkdir -p "${f%.tar.gz}" && tar -xzf "$f" -C "${f%.tar.gz}"; done

INPUT: count lines of typescript code excluding node_modules
OUTPUT: find . -type f \( -name '*.ts' -o -name '*.tsx' \) -not -path '*/node_modules/*' -exec wc -l '{}' +

INPUT: copy current branch name to clipboard
OUTPUT: git rev-parse --abbrev-ref HEAD | tr -d '\n' | wl-copy

INPUT: replace all tabs with 2 spaces in every js file under src
OUTPUT: find src -type f -name '*.js' -exec sed -i 's/\t/  /g' '{}' +

INPUT: search for TODO comments in this repo, ignoring git directory
OUTPUT: grep -rn --exclude-dir=.git 'TODO' .

INPUT: download a url to disk
OUTPUT: curl -fLo file.bin https://example.com/file.bin"#
        }
        Shell::Fish => {
            r#"INPUT: find rust files modified in the last week
OUTPUT: find . -type f -name '*.rs' -mtime -7

INPUT: kill whatever is listening on port 3000
OUTPUT: kill -9 (lsof -t -i:3000)

INPUT: 10 largest files under this directory
OUTPUT: du -ah . 2>/dev/null | sort -rh | head -10

INPUT: tmp gb
OUTPUT: du -x --si -d1 /tmp 2>/dev/null | sort -hr | head -20

INPUT: largest folders here
OUTPUT: du -x --si -d1 . 2>/dev/null | sort -hr | head -20

INPUT: pretty-print package.json dependencies
OUTPUT: jq '.dependencies' package.json

INPUT: follow nginx logs
OUTPUT: journalctl -u nginx -b -f --no-hostname

INPUT: extract every .tar.gz here into its own folder
OUTPUT: for f in *.tar.gz; set -l d (string replace -r '\.tar\.gz$' '' -- "$f"); mkdir -p "$d"; and tar -xzf "$f" -C "$d"; end

INPUT: count lines of typescript code excluding node_modules
OUTPUT: find . -type f \( -name '*.ts' -o -name '*.tsx' \) -not -path '*/node_modules/*' -exec wc -l '{}' +

INPUT: copy current branch name to clipboard
OUTPUT: git rev-parse --abbrev-ref HEAD | tr -d '\n' | wl-copy

INPUT: replace all tabs with 2 spaces in every js file under src
OUTPUT: find src -type f -name '*.js' -exec sed -i 's/\t/  /g' '{}' +

INPUT: search for TODO comments in this repo, ignoring git directory
OUTPUT: grep -rn --exclude-dir=.git 'TODO' .

INPUT: download a url to disk
OUTPUT: curl -fLo file.bin https://example.com/file.bin"#
        }
    }
}

pub struct DirectivesArgs<'a> {
    pub qshrc_prompt: &'a str,
    pub retry: bool,
    pub refine: bool,
    pub explain: bool,
    pub alts: u32,
}

pub fn extra_directives(args: &DirectivesArgs<'_>) -> String {
    let mut out = String::new();

    if !args.qshrc_prompt.is_empty() {
        out.push_str("\n\nPROJECT DIRECTIVES (from .qshrc)\n");
        out.push_str(args.qshrc_prompt);
    }

    if args.retry {
        push_directive(&mut out, RETRY_DIRECTIVE);
    } else if args.refine {
        push_directive(&mut out, REFINE_DIRECTIVE);
    }

    if args.explain {
        push_directive(&mut out, EXPLAIN_DIRECTIVE);
    }

    if args.alts > 1 && !args.retry && !args.refine {
        let directive = ALTS_DIRECTIVE_TEMPLATE
            .replace("__ALTS__", &args.alts.to_string())
            .replace("__MINUS_ONE__", &(args.alts - 1).to_string())
            .replace("__PLUS_ONE__", &(args.alts + 1).to_string());
        push_directive(&mut out, &directive);
    }

    out
}

fn push_directive(out: &mut String, directive: &str) {
    out.push_str("\n\n");
    out.push_str(directive.trim_end());
}

pub fn max_tokens(mode: Mode, explain: bool, alts: u32) -> u32 {
    use crate::config::*;
    let mut t = match mode {
        Mode::Smart => TOKENS_SMART,
        Mode::Fast => TOKENS_FAST,
    };
    if explain {
        t += TOKENS_EXPLAIN_BONUS;
    }
    if alts > 1 && mode != Mode::Smart {
        t += alts * TOKENS_PER_ALT;
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Mode;

    fn env_info() -> EnvInfo {
        EnvInfo {
            os_pretty: "TestOS".into(),
            os_kind: "Test".into(),
            pkg_rule: "no-install".into(),
            clipboard_line: "no-clip".into(),
            clipboard_tools: "none".into(),
        }
    }

    #[test]
    fn placeholders_substituted_for_zsh() {
        let sys = system_prompt(&env_info(), Shell::Zsh);
        assert!(sys.contains("deterministic zsh command generator"));
        assert!(sys.contains("Shell: zsh with EXTENDED_GLOB, NOMATCH, AUTOCD."));
        assert!(sys.contains("TestOS"));
        assert!(sys.contains("no-clip"));
        assert!(sys.contains("no-install"));
        assert!(!sys.contains("__OS_PRETTY__"));
        assert!(!sys.contains("__SHELL_NAME__"));
    }

    #[test]
    fn placeholders_substituted_for_bash() {
        let sys = system_prompt(&env_info(), Shell::Bash);
        assert!(sys.contains("deterministic bash command generator"));
        assert!(sys.contains("the user's bash shell"));
        assert!(sys.contains("Shell: bash with standard Bash semantics."));
        assert!(!sys.contains("EXTENDED_GLOB"));
        assert!(!sys.contains("__SHELL_DESCRIPTION__"));
    }

    #[test]
    fn placeholders_substituted_for_fish() {
        let sys = system_prompt(&env_info(), Shell::Fish);
        assert!(sys.contains("deterministic fish command generator"));
        assert!(sys.contains("the user's fish shell"));
        assert!(sys.contains("Shell: fish with native Fish syntax"));
        assert!(sys.contains("Command substitution is `(...)`, not `$(...)`"));
        assert!(sys.contains("There is no `do`, `then`, or `fi`."));
        assert!(sys.contains("OUTPUT: kill -9 (lsof -t -i:3000)"));
        assert!(sys.contains("for f in *.tar.gz; set -l d"));
        assert!(!sys.contains("__SHELL_SYNTAX__"));
        assert!(!sys.contains("__SHELL_EXAMPLES__"));
    }

    #[test]
    fn directives_loaded_from_prompt_files() {
        let directives = extra_directives(&DirectivesArgs {
            qshrc_prompt: "Prefer cargo nextest.",
            retry: true,
            refine: false,
            explain: true,
            alts: 4,
        });
        assert!(directives.contains("PROJECT DIRECTIVES"));
        assert!(directives.contains("RETRY MODE"));
        assert!(directives.contains("EXPLAIN MODE OVERRIDE"));
        assert!(!directives.contains("ALTS MODE OVERRIDE"));
    }

    #[test]
    fn alts_template_substitutes_counts() {
        let directives = extra_directives(&DirectivesArgs {
            qshrc_prompt: "",
            retry: false,
            refine: false,
            explain: false,
            alts: 4,
        });
        assert!(directives.contains("Produce exactly 4 distinct alternative commands"));
        assert!(directives.contains("Not 3, not 5"));
        assert!(!directives.contains("__ALTS__"));
    }

    #[test]
    fn tokens_fast_default() {
        assert_eq!(max_tokens(Mode::Fast, false, 1), 500);
    }

    #[test]
    fn tokens_explain_bonus() {
        assert_eq!(max_tokens(Mode::Fast, true, 1), 700);
    }

    #[test]
    fn tokens_alts_fast() {
        // 500 + 4 * 800
        assert_eq!(max_tokens(Mode::Fast, false, 4), 500 + 4 * 800);
    }

    #[test]
    fn tokens_smart_alts_no_bonus() {
        // Smart already has huge headroom; per-alt overhead skipped.
        assert_eq!(max_tokens(Mode::Smart, false, 4), 16_000);
    }
}
