#!/usr/bin/env bash
# Installs the agent as a service for the current user, checks that it runs and starts its
# Syncthing, that installing again restarts it, uninstalls it and checks that Syncthing stopped
# too. For CI runners only: it
# changes the user's services. Usage: scripts/service-smoke.sh <dir with the agent binaries>
set -euo pipefail

bin=$1
data=${RUNNER_TEMP:-/tmp}/stellwerk-service-smoke
agent=$bin/stellwerk-agent
case "$(uname -s)" in
  Linux) os=linux ;;
  Darwin) os=macos ;;
  *) os=windows; agent=$bin/stellwerk-agent.exe ;;
esac

wait_for() { # <seconds> <description> <command…>
  local seconds=$1 what=$2; shift 2
  for _ in $(seq "$seconds"); do
    if "$@" >/dev/null 2>&1; then echo "ok: $what"; return 0; fi
    sleep 1
  done
  echo "timed out: $what"; return 1
}

syncthing_running() {
  if [ "$os" = windows ]; then
    tasklist //FI "IMAGENAME eq syncthing.exe" | grep -qi syncthing.exe
  else
    pgrep -f "$data/syncthing" >/dev/null
  fi
}
syncthing_stopped() { ! syncthing_running; }

service_running() {
  case $os in
    linux) systemctl --user is-active --quiet stellwerk-agent.service ;;
    macos) launchctl print "gui/$(id -u)/io.github.firsttris.stellwerk.agent" | grep -q "state = running" ;;
    windows) schtasks //Query //TN 'Stellwerk\Agent' //FO LIST 2>/dev/null | grep -qi running ;;
  esac
}

# The process ID of the running agent; empty or 0 when none runs.
agent_pid() {
  case $os in
    linux) systemctl --user show -p MainPID --value stellwerk-agent.service ;;
    macos) launchctl print "gui/$(id -u)/io.github.firsttris.stellwerk.agent" | sed -n 's/^[[:space:]]*pid = //p' | head -1 ;;
    windows) tasklist //FI "IMAGENAME eq stellwerk-agent-background.exe" //FO CSV //NH | cut -s -d, -f2 | tr -d '"' | head -1 ;;
  esac
}
agent_restarted() { # <pid before>
  local now; now=$(agent_pid)
  [ -n "$now" ] && [ "$now" != 0 ] && [ "$now" != "$1" ]
}

show_logs() {
  echo "--- logs"
  case $os in
    linux) journalctl --user -u stellwerk-agent.service --no-pager | tail -40 || true ;;
    *) tail -40 "$data/agent.log" || true ;;
  esac
}
trap show_logs ERR

rm -rf "$data"
# A Zentrale that is not there: the agent keeps retrying, which is all this needs.
"$agent" --data-dir "$data" join --server http://127.0.0.1:9 ABCD-EFGH-JKLM
"$agent" --data-dir "$data" service install
wait_for 30 "service running" service_running
wait_for 180 "Syncthing started by the agent" syncthing_running
# Installing again, as after replacing the binary, restarts the agent
before=$(agent_pid)
"$agent" --data-dir "$data" service install
wait_for 60 "agent restarted by a second install" agent_restarted "$before"
wait_for 180 "Syncthing running after the restart" syncthing_running
"$agent" --data-dir "$data" service uninstall
wait_for 60 "Syncthing stopped with the agent" syncthing_stopped
if service_running; then echo "service still running"; exit 1; fi
echo "service smoke test passed on $os"
