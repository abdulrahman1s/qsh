# qsh demo tape (VHS)
#
# Render with:   vhs demo.tape
# Produces:      demo.gif
#
# Requirements:
#   - qsh on PATH
#   - At least one provider configured (GEMINI_API_KEY, ANTHROPIC_API_KEY,
#     OPENAI_API_KEY, or a local Ollama model). Demo will hit the live API.
#   - vhs >= 0.7, zsh.
#
# Tuning notes:
#   - Sleeps after `Enter` allow time for the streamed response. If your
#     provider is slow, bump them. If fast mode is near-instant, trim them.
#   - The retry demo's first attempt MUST reliably fail. The setup is a
#     `deploy.sh` script with no executable bit. The natural first guess
#     (`./deploy.sh`) bombs with `zsh: permission denied: ./deploy.sh`,
#     and the retry pivots to the canonical fix —
#     `chmod +x deploy.sh && ./deploy.sh` — which actually runs the
#     script. Universally relatable: every dev has hit this exact wall.

Output demo.gif

Require qsh

Set Shell        zsh
Set FontSize     16
Set Width        1200
Set Height       720
Set Padding      24
Set Theme        "Catppuccin Mocha"
Set TypingSpeed  55ms
Set PlaybackSpeed 1.0

# ---------- setup (hidden) ----------
# Single Hide/Show pair — VHS doesn't play well with multiple toggles,
# so everything that must not appear in the GIF goes here.
Hide
Type "export PS1='%F{cyan}❯%f ' && cd $(mktemp -d) && clear"   Enter
Type `eval "$(qsh init zsh)"`                                  Enter
# Wipe ~/.cache/qsh so identical queries on re-renders hit the
# model fresh instead of replaying a cached candidate.
Type "? --clear-cache"                                         Enter
Sleep 400ms
# Fixture for the retry demo: a real deploy script without the exec
# bit. The first attempt at `./deploy.sh` will fail with permission
# denied; the retry's `chmod +x …` fix actually runs the script.
Type `printf '%s\n' '#!/usr/bin/env bash' 'echo "deployed to staging ✓"' > deploy.sh`   Enter
Type "clear"                                                   Enter
Sleep 500ms
Show

# ---------- demo 1: a basic ask ----------
Type "? find large files over 100MB under /var/log"
Sleep 800ms
Enter
# wait for stream + confirm prompt
Sleep 4s
Type "n"
Sleep 1500ms

# ---------- demo 2: the killer feature — retry on failure ----------
# Setup happens up top (hidden). Here we just run the script and let
# the failure → retry → success arc play out.
Type "? run deploy.sh"
Sleep 800ms
Enter
Sleep 4s
Type "y"
# eval runs the bad command, stderr is captured for retry replay
Sleep 2500ms

# Bare `?` within 10 minutes replays the original intent + the
# failed attempt + its stderr. Model sees `permission denied` and
# pivots to `chmod +x deploy.sh && ./deploy.sh`.
Type "?"
Sleep 600ms
Enter
Sleep 5s
Type "y"
Sleep 2500ms

# ---------- demo 3: refine — iterate without restarting ----------
Type "clear"   Enter
Sleep 400ms

Type "? show listening ports"
Sleep 800ms
Enter
Sleep 5s

# `r` re-prompts with a follow-up directive, preserving original intent.
Type "r"
Sleep 600ms
Type "only ipv6, skip ssh"
Enter
Sleep 5s
Type "n"
Sleep 2s

# ---------- outro ----------
Type "clear && echo '?  =  fast mode' && echo '?? =  smart mode (reasoning)' && echo 'press ? again after a failure to retry'"
Enter
Sleep 7s
