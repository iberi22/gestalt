#!/usr/bin/env bash
# Gestalt Leaf TUI Dashboard: 4 Panes for N Repos
# Design: SWAL pipeline router central (Hermes) vs 4 leaf TUIs

SESSION="gestalt-tui"

# Ensure tmux is available
if ! command -v tmux &> /dev/null; then
    echo "tmux is required to run gestalt-tui.sh"
    exit 1
fi

# Kill existing session if present
tmux kill-session -t "$SESSION" 2>/dev/null || true

# Helper command for pane watching
watcher_cmd() {
    local repo="$1"
    echo "watch -n 20 'gestalt bus replay --since 1h --project $repo --json 2>/dev/null | tail -20; gh pr list --repo iberi22/$repo --state open 2>/dev/null | head -5'"
}

# Create a new detached session with Pane 0 (top-left: gara-g)
tmux new-session -d -s "$SESSION" -n "Dashboard" "$(watcher_cmd "gara-g")"

# Split window horizontally to create right pane (top-right: hosteler-ia)
tmux split-window -h -t "$SESSION:0" "$(watcher_cmd "hosteler-ia")"

# Split top-left pane vertically to create bottom-left pane (bottom-left: xavier)
tmux split-window -v -t "$SESSION:0.0" "$(watcher_cmd "xavier")"

# Split top-right pane vertically to create bottom-right pane (bottom-right: OrionHealth)
tmux split-window -v -t "$SESSION:0.1" "$(watcher_cmd "OrionHealth")"

# Equalize pane sizes to form a clean 2x2 grid
tmux select-layout -t "$SESSION:0" tiled

# Attach if interactive session, otherwise notify
if [ -t 1 ]; then
    tmux attach-session -t "$SESSION"
else
    echo "Gestalt TUI dashboard session '$SESSION' created with 4 panes."
fi
