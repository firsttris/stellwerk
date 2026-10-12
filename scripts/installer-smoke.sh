#!/usr/bin/env bash
# Installs the agent with the Windows installer silently, checks that the task at logon runs
# and started Syncthing, uninstalls it and checks that everything stopped and is gone. For CI
# runners only. Usage: scripts/installer-smoke.sh <setup.exe>
set -euo pipefail
# Git Bash would rewrite /S, /SERVER=http://… and the like as paths.
export MSYS_NO_PATHCONV=1

setup=$1
dir="$LOCALAPPDATA/Programs/Stellwerk"
data="$LOCALAPPDATA/stellwerk-agent"

wait_for() { # <seconds> <description> <command…>
  local seconds=$1 what=$2; shift 2
  for _ in $(seq "$seconds"); do
    if "$@" >/dev/null 2>&1; then echo "ok: $what"; return 0; fi
    sleep 1
  done
  echo "timed out: $what"; tail -40 "$data/agent.log" || true; return 1
}
task_running() { schtasks /Query /TN 'Stellwerk\Agent' /FO LIST | grep -qi running; }
task_gone() { ! schtasks /Query /TN 'Stellwerk\Agent' >/dev/null 2>&1; }
syncthing_running() { tasklist /FI "IMAGENAME eq syncthing.exe" | grep -qi syncthing.exe; }
syncthing_stopped() { ! syncthing_running; }

# A Zentrale that is not there: the agent keeps retrying, which is all this needs.
"$setup" /S /SERVER=http://127.0.0.1:9 /CODE=ABCD-EFGH-JKLM || {
  echo "installer failed"; cat "$dir/install-error.log" || true; exit 1
}
test -f "$dir/stellwerk-agent.exe" && test -f "$dir/stellwerk-agent-background.exe"
grep -q ABCD-EFGH-JKLM "$data/agent.json"
wait_for 30 "task running" task_running
wait_for 180 "Syncthing started by the agent" syncthing_running

"$dir/uninstall.exe" /S
# The uninstaller copies itself away and returns at once.
wait_for 60 "task removed" task_gone
wait_for 60 "Syncthing stopped with the agent" syncthing_stopped
wait_for 30 "programs removed" test ! -e "$dir/stellwerk-agent.exe"
echo "installer smoke test passed"
