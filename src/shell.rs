use crate::cli::{Cli, InitArgs, Shell};
use clap::CommandFactory;
use clap_complete::{Shell as ClapShell, generate};

pub fn init(args: InitArgs) -> i32 {
    let wrapper = match args.shell {
        Shell::Bash => BASH_INIT,
        Shell::Fish => FISH_INIT,
        Shell::Zsh => ZSH_INIT,
    };
    println!("{}", wrapper);
    println!("{}", render_completion(args.shell));
    0
}

fn render_completion(shell: Shell) -> String {
    let mut cmd = Cli::command();
    let mut buf: Vec<u8> = Vec::new();
    let clap_shell = match shell {
        Shell::Bash => ClapShell::Bash,
        Shell::Fish => ClapShell::Fish,
        Shell::Zsh => ClapShell::Zsh,
    };
    generate(clap_shell, &mut cmd, "qsh", &mut buf);
    let mut script = String::from_utf8(buf).unwrap_or_default();

    // Mirror completion onto the `?` / `??` aliases so `<tab>` works there too.
    // zsh: alias `?` resolves to `noglob qsh`; `compdef` registers _qsh for the alias name.
    // bash: alias `?` runs `__qsh_pre_noglob; qsh`; completion keys off the first word.
    // fish: `?` and `??` are functions; `--wraps qsh` delegates completion.
    let aliasing = match shell {
        Shell::Zsh => "\ncompdef _qsh '?' '??'\n",
        Shell::Bash => "\ncomplete -F _qsh -o nosort -o bashdefault -o default '?' '??'\n",
        Shell::Fish => "\ncomplete -c '?' --wraps qsh\ncomplete -c '??' --wraps qsh\n",
    };
    script.push_str(aliasing);
    script
}

// Zsh wrapper. The strategy:
//   - `qsh` calls `qsh generate ...` and captures the accepted command
//     from stdout. Everything else (status, streaming, errors) goes
//     through stderr unchanged.
//   - `print -s --` pushes the command into the user's shell history so
//     up-arrow recall works naturally.
//   - eval runs the command in the *current* zsh process. Stderr is teed
//     into a tempfile so retry-state recording sees what failed.
//   - On exit we call `qsh record` so the JSONL is updated.
// The `?` alias gets `noglob` so the zsh glob doesn't eat the literal.
const ZSH_INIT: &str = r#"# qsh zsh integration. Source this from your zshrc:
#   eval "$(qsh init zsh)"

qsh() {
  case "$1" in
    generate|record|init|-h|--help|-V|--version)
      command qsh "$@"
      return $?
      ;;
  esac

  setopt LOCAL_OPTIONS LOCAL_TRAPS NO_NOTIFY NO_MULTIOS PIPE_FAIL NO_XTRACE NO_VERBOSE TYPESET_SILENT
  TRAPCHLD() { :; }

  local cmd_file err_file rc=0
  cmd_file=$(mktemp) || return 1
  err_file=$(mktemp) || { rm -f -- "$cmd_file"; return 1; }
  trap 'rm -f -- "$cmd_file" "$err_file" 2>/dev/null' EXIT

  command qsh generate --shell zsh "$@" >"$cmd_file"
  rc=$?
  if (( rc != 0 )); then
    return $rc
  fi

  local cmd
  cmd=$(< "$cmd_file")
  [[ -z "$cmd" ]] && return 0

  local hist_cmd="${cmd% [#]*}"
  hist_cmd="${hist_cmd%"${hist_cmd##*[![:space:]]}"}"
  [[ -n "$hist_cmd" ]] && print -s -- "$hist_cmd"

  { eval "$cmd" 3>&1 1>&4 2>&3 | tee -- "$err_file" >&2
    rc=${pipestatus[1]}
  } 4>&1

  local last_task=""
  [[ -f "$cmd_file.task" ]] && last_task=$(< "$cmd_file.task")

  command qsh record --cmd "$cmd" --status "$rc" --stderr-file "$err_file" --original-task "$last_task" >/dev/null 2>&1 || true
  return $rc
}

alias "?"="noglob qsh"
alias "??"="noglob qsh --smart"
"#;

// Bash wrapper. Bash has no zsh-style `noglob` precommand, so the `?`
// aliases expand to a tiny pre-step that disables globbing before Bash
// expands the natural-language arguments. `qsh` restores the caller's
// globbing option immediately, before the generated command is evaluated.
const BASH_INIT: &str = r#"# qsh bash integration. Source this from your bashrc:
#   eval "$(qsh init bash)"

__qsh_pre_noglob() {
  case $- in
    *f*) __QSH_HAD_NOGLOB=1 ;;
    *) __QSH_HAD_NOGLOB=0 ;;
  esac
  set -f
}

__qsh_restore_glob() {
  if [[ ${__QSH_HAD_NOGLOB+x} ]]; then
    if [[ $__QSH_HAD_NOGLOB == 0 ]]; then
      set +f
    fi
    unset __QSH_HAD_NOGLOB
  fi
}

qsh() {
  __qsh_restore_glob

  case "$1" in
    generate|record|init|-h|--help|-V|--version)
      command qsh "$@"
      return $?
      ;;
  esac

  local cmd_file err_file rc=0
  cmd_file=$(mktemp) || return 1
  err_file=$(mktemp) || { rc=$?; rm -f -- "$cmd_file"; return "$rc"; }

  command qsh generate --shell bash "$@" >"$cmd_file"
  rc=$?
  if (( rc != 0 )); then
    rm -f -- "$cmd_file" "$err_file"
    return "$rc"
  fi

  local cmd
  cmd=$(<"$cmd_file")
  if [[ -z "$cmd" ]]; then
    rm -f -- "$cmd_file" "$err_file"
    return 0
  fi

  local hist_cmd="${cmd% [#]*}"
  hist_cmd="${hist_cmd%"${hist_cmd##*[![:space:]]}"}"
  [[ -n "$hist_cmd" ]] && history -s -- "$hist_cmd"

  { eval "$cmd" 3>&1 1>&4 2>&3 | tee -- "$err_file" >&2
    rc=${PIPESTATUS[0]}
  } 4>&1

  local last_task=""
  [[ -f "$cmd_file.task" ]] && last_task=$(<"$cmd_file.task")

  command qsh record --cmd "$cmd" --status "$rc" --stderr-file "$err_file" --original-task "$last_task" >/dev/null 2>&1 || true
  rm -f -- "$cmd_file" "$err_file"
  return "$rc"
}

alias "?"="__qsh_pre_noglob; qsh"
alias "??"="__qsh_pre_noglob; qsh --smart"
"#;

// Fish wrapper. Fish uses native syntax for the function body and defines
// `?`/`??` as functions instead of abbreviations so the command line is not
// rewritten before execution.
const FISH_INIT: &str = r#"# qsh fish integration. Source this from your config.fish:
#   qsh init fish | source

function qsh
    set -l subcmd ""
    if set -q argv[1]
        set subcmd $argv[1]
    end

    switch "$subcmd"
        case generate record init -h --help -V --version
            command qsh $argv
            return $status
    end

    set -l cmd_file (mktemp)
    or return 1
    set -l err_file (mktemp)
    or begin
        set -l rc $status
        rm -f -- "$cmd_file"
        return $rc
    end

    command qsh generate --shell fish $argv >"$cmd_file"
    set -l rc $status
    if test $rc -ne 0
        rm -f -- "$cmd_file" "$err_file"
        return $rc
    end

    set -l cmd (cat "$cmd_file")
    if test -z "$cmd"
        rm -f -- "$cmd_file" "$err_file"
        return 0
    end

    set -l hist_cmd (string replace -r ' [#].*$' '' -- "$cmd" | string trim -r)
    if test -n "$hist_cmd"
        history append -- "$hist_cmd" >/dev/null 2>&1
    end

    begin
        eval "$cmd" 3>&1 1>&4 2>&3 | tee -- "$err_file" >&2
        set rc $pipestatus[1]
    end 4>&1

    set -l last_task ""
    if test -f "$cmd_file.task"
        set last_task (cat "$cmd_file.task")
    end

    command qsh record --cmd "$cmd" --status "$rc" --stderr-file "$err_file" --original-task "$last_task" >/dev/null 2>&1; or true
    rm -f -- "$cmd_file" "$err_file"
    return $rc
end

abbr --erase '?' >/dev/null 2>&1
abbr --erase '??' >/dev/null 2>&1
functions --erase '?' >/dev/null 2>&1
functions --erase '??' >/dev/null 2>&1

function '?' --description 'qsh fast mode'
    qsh $argv
end

function '??' --description 'qsh smart mode'
    qsh --smart $argv
end
"#;

#[cfg(test)]
mod tests {
    use super::{BASH_INIT, FISH_INIT, ZSH_INIT, render_completion};
    use crate::cli::Shell;

    #[test]
    fn zsh_init_does_not_reference_bash_glob_restore_helper() {
        assert!(!ZSH_INIT.contains("__qsh_restore_glob"));
        assert!(ZSH_INIT.contains(r#"alias "?"="noglob qsh""#));
    }

    #[test]
    fn bash_init_keeps_glob_restore_helper() {
        assert!(BASH_INIT.contains("__qsh_restore_glob()"));
        assert!(BASH_INIT.contains("qsh() {\n  __qsh_restore_glob\n\n  case \"$1\" in"));
        assert!(BASH_INIT.contains(r#"alias "?"="__qsh_pre_noglob; qsh""#));
    }

    #[test]
    fn fish_init_uses_fish_shell_context_and_question_mark_functions() {
        assert!(FISH_INIT.contains("command qsh generate --shell fish $argv"));
        assert!(FISH_INIT.contains("abbr --erase '?'"));
        assert!(FISH_INIT.contains("function '?' --description 'qsh fast mode'"));
        assert!(FISH_INIT.contains("function '??' --description 'qsh smart mode'"));
        assert!(!FISH_INIT.contains("abbr --add"));
        assert!(!FISH_INIT.contains("__qsh_restore_glob"));
    }

    #[test]
    fn zsh_completion_registers_question_mark_aliases() {
        let script = render_completion(Shell::Zsh);
        assert!(script.contains("_qsh()"));
        assert!(script.contains("compdef _qsh '?' '??'"));
    }

    #[test]
    fn bash_completion_registers_question_mark_aliases() {
        let script = render_completion(Shell::Bash);
        assert!(script.contains("_qsh()"));
        assert!(script.contains("complete -F _qsh -o nosort -o bashdefault -o default '?' '??'"));
    }

    #[test]
    fn fish_completion_wraps_question_mark_functions() {
        let script = render_completion(Shell::Fish);
        assert!(script.contains("complete -c qsh"));
        assert!(script.contains("complete -c '?' --wraps qsh"));
        assert!(script.contains("complete -c '??' --wraps qsh"));
    }
}
