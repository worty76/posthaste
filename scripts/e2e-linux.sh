#!/usr/bin/env bash
# End-to-end check of the capture flow on Linux/X11, in a throwaway X server.
#
#   global shortcut -> capture window focused -> type -> Enter
#   -> note in SQLite -> window hidden -> previous app focused again
#
# Needs: xvfb, openbox, xdotool, x11-apps (xedit stands in for "the app you were working in"), sqlite3, dbus, imagemagick (import).
# Usage: scripts/e2e-linux.sh [path/to/posthaste-binary] [screenshot-dir]
set -u
BIN="${1:-src-tauri/target/debug/posthaste}"
SHOTS="${2:-}"
export DISPLAY=:97
WORK="$(mktemp -d)"
export XDG_DATA_HOME="$WORK/data" XDG_CONFIG_HOME="$WORK/config" XDG_CACHE_HOME="$WORK/cache"
DB="$XDG_DATA_HOME/dev.posthaste.desktop/posthaste.db"
unset WAYLAND_DISPLAY; export XDG_SESSION_TYPE=x11
export WEBKIT_DISABLE_DMABUF_RENDERER=1
PASS=0; FAIL=0
ok()   { PASS=$((PASS+1)); echo "  ok    $1"; }
bad()  { FAIL=$((FAIL+1)); echo "  FAIL  $1"; }
check(){ if eval "$2"; then ok "$1"; else bad "$1"; fi; }
shot() { [ -n "$SHOTS" ] && mkdir -p "$SHOTS" && import -window root "$SHOTS/$1.png" 2>/dev/null; return 0; }
active(){ xdotool getactivewindow 2>/dev/null; }
capture_visible(){ xdotool search --onlyvisible --name '^Posthaste$' 2>/dev/null | head -1; }
wait_for(){ for _ in $(seq 1 "${2:-50}"); do eval "$1" && return 0; sleep 0.1; done; return 1; }
sql()  { sqlite3 "$DB" "$1" 2>/dev/null; }
# The newest migration's version, read from the source of truth so this
# script does not go stale when the next migration lands.
LATEST_SCHEMA=$(grep -oE 'version: [0-9]+' src-tauri/crates/core/src/migrations.rs | tail -1 | grep -oE '[0-9]+')

Xvfb $DISPLAY -screen 0 1440x900x24 >/dev/null 2>&1 & PIDS="$!"
sleep 1
eval "$(dbus-launch --sh-syntax)"; PIDS="$PIDS $DBUS_SESSION_BUS_PID"
openbox >/dev/null 2>&1 & PIDS="$PIDS $!"
command -v picom >/dev/null && { picom --backend xrender >/dev/null 2>&1 & PIDS="$PIDS $!"; }
xsetroot -solid '#3b4252' 2>/dev/null
xedit -geometry 500x340+60+60 -title editor >/dev/null 2>&1 & PIDS="$PIDS $!"
trap 'kill $APP $PIDS 2>/dev/null; sleep 0.3; rm -rf "$WORK" 2>/dev/null' EXIT
sleep 1

"$BIN" >"$WORK/app.log" 2>&1 & APP=$!
echo "Startup"
check "main window appears" "wait_for 'xdotool search --onlyvisible --name \"^Posthaste\$\" >/dev/null 2>&1' 150"
check "database file created" "wait_for '[ -f \"$DB\" ]' 50"
check "schema is at version $LATEST_SCHEMA" "[ \"\$(sql 'PRAGMA user_version')\" = \"$LATEST_SCHEMA\" ]"
check "no shortcut registration failure logged" "! grep -qi 'not registered' \"$WORK/app.log\""
sleep 1.5; shot 01-inbox-empty

echo "Capture"
EDITOR_WIN="$(xdotool search --name '^editor$' | head -1)"
xdotool windowactivate --sync "$EDITOR_WIN"; sleep 0.3
echo "        (editor window $EDITOR_WIN, active $(active))"
check "editor is focused before capture" "[ \"\$(active)\" = \"$EDITOR_WIN\" ]"
T0=$(date +%s%N)
xdotool key ctrl+shift+space
check "capture window appears on the global shortcut" "wait_for '[ -n \"\$(capture_visible)\" ]' 50"
echo "        (visible after $(( ($(date +%s%N) - T0) / 1000000 )) ms, including polling)"
CAP="$(capture_visible)"
check "capture window has focus" "wait_for '[ \"\$(active)\" = \"$CAP\" ]' 30"
eval "$(xdotool getwindowgeometry --shell "$CAP")"; echo "        (geometry ${WIDTH}x${HEIGHT} at $X,$Y)"
CX=$((X + WIDTH/2)); CY=$((Y + HEIGHT/2))
check "capture window is centred on the display" "[ $((CX-720)) -ge -3 ] && [ $((CX-720)) -le 3 ] && [ $((CY-450)) -ge -30 ] && [ $((CY-450)) -le 30 ]"
sleep 0.4
xdotool type --delay 15 "Investigate PostgreSQL indexing strategy."
sleep 0.6; shot 02-capture
check "typing reached the input and was drafted to disk" "[ \"\$(sql 'SELECT content FROM drafts')\" = 'Investigate PostgreSQL indexing strategy.' ]"
xdotool key Return
check "note is in SQLite after Enter" "wait_for '[ \"\$(sql \"SELECT count(*) FROM notes WHERE content = \\\"Investigate PostgreSQL indexing strategy.\\\" AND status = \\\"inbox\\\"\")\" = 1 ]' 30"
check "capture window is hidden after saving" "wait_for '[ -z \"\$(capture_visible)\" ]' 30"
check "focus returned to the editor" "wait_for '[ \"\$(active)\" = \"$EDITOR_WIN\" ]' 30"
check "draft was cleared by the save" "[ \"\$(sql 'SELECT count(*) FROM drafts')\" = 0 ]"
check "source application recorded" "[ -n \"\$(sql 'SELECT source FROM notes LIMIT 1')\" ]"

echo "Draft protection"
xdotool key ctrl+shift+space; wait_for '[ -n "$(capture_visible)" ]' 50; sleep 0.4
xdotool type --delay 15 "half a thought"
xdotool key Escape
check "Escape hides the window" "wait_for '[ -z \"\$(capture_visible)\" ]' 30"
check "Escape keeps the text as a draft" "wait_for '[ \"\$(sql \"SELECT content FROM drafts\")\" = \"half a thought\" ]' 20"
check "Escape did not save a note" "[ \"\$(sql 'SELECT count(*) FROM notes')\" = 1 ]"
check "focus returned to the editor after Escape" "wait_for '[ \"\$(active)\" = \"$EDITOR_WIN\" ]' 30"
xdotool key ctrl+shift+space; wait_for '[ -n "$(capture_visible)" ]' 50; sleep 0.5; shot 03-draft-restored
xdotool key Return; sleep 0.2
check "empty-looking Enter saved the restored draft" "wait_for '[ \"\$(sql \"SELECT count(*) FROM notes WHERE content = \\\"half a thought\\\"\")\" = 1 ]' 30"

echo "Empty input and multi-line"
xdotool key ctrl+shift+space; wait_for '[ -n "$(capture_visible)" ]' 50; sleep 0.4
xdotool key Return; sleep 0.4
check "Enter on empty input saves nothing" "[ \"\$(sql 'SELECT count(*) FROM notes')\" = 2 ]"
check "Enter on empty input keeps the window open" "[ -n \"\$(capture_visible)\" ]"
xdotool type --delay 15 "line one"; xdotool key shift+Return; xdotool type --delay 15 "line two"
xdotool key ctrl+Return
check "Ctrl+Enter saves a multi-line note" "wait_for '[ \"\$(sql \"SELECT count(*) FROM notes WHERE content = \\\"line one\\\" || char(10) || \\\"line two\\\"\")\" = 1 ]' 30"
sleep 0.3
check "Ctrl+Enter keeps the window open for another" "[ -n \"\$(capture_visible)\" ]"
xdotool key Escape; wait_for '[ -z "$(capture_visible)" ]' 30

echo "Today window"
today_visible(){ xdotool search --onlyvisible --name '^Posthaste Today$' 2>/dev/null | head -1; }
# Two open notes from days ago: they are what the glance carries over.
DAY_MS=$(( $(date -d '3 days ago' +%s) * 1000 ))
for i in 1 2; do
  sql "INSERT INTO notes (id, content, status, created_at, updated_at)
       VALUES (hex(randomblob(8)), 'carried task $i', 'inbox', $DAY_MS, $DAY_MS)"
done
check "two carried notes were seeded" "[ \"\$(sql \"SELECT count(*) FROM notes WHERE content LIKE 'carried task%' AND created_at < $((DAY_MS + 86400000))\")\" = 2 ]"
xdotool windowactivate --sync "$EDITOR_WIN"; sleep 0.3
xdotool key ctrl+alt+g
check "today window appears on the shortcut" "wait_for '[ -n \"\$(today_visible)\" ]' 50"
TODAY="$(today_visible)"
check "today window has focus" "wait_for '[ \"\$(active)\" = \"$TODAY\" ]' 30"
check "the capture window did not open with it" "[ -z \"\$(capture_visible)\" ]"
sleep 0.5; shot 06-today
xdotool key Escape
check "Escape hides the today window" "wait_for '[ -z \"\$(today_visible)\" ]' 30"
check "focus returned to the editor after the glance" "wait_for '[ \"\$(active)\" = \"$EDITOR_WIN\" ]' 30"
echo "Note details"
details_visible(){ xdotool search --onlyvisible --name '^Posthaste Note$' 2>/dev/null | head -1; }
details_saved(){ [ "$(sql "SELECT count(*) FROM notes WHERE details = 'agenda for the review'")" = 1 ]; }
xdotool windowactivate --sync "$EDITOR_WIN"; sleep 0.3
xdotool key ctrl+alt+g; wait_for '[ -n "$(today_visible)" ]' 50; sleep 0.5
xdotool key Return
check "Enter in Today opens the details window" "wait_for '[ -n \"\$(details_visible)\" ]' 50"
check "Today steps aside while details are open" "wait_for '[ -z \"\$(today_visible)\" ]' 30"
DETAILS="$(details_visible)"
check "details window has focus" "wait_for '[ \"\$(active)\" = \"$DETAILS\" ]' 30"
sleep 0.4
xdotool type --delay 15 "agenda for the review"; sleep 0.3
shot 07-details
xdotool key Escape
check "Escape hides the details window" "wait_for '[ -z \"\$(details_visible)\" ]' 30"
check "Today comes back after details" "wait_for '[ -n \"\$(today_visible)\" ]' 30"
check "the description was saved before closing" "wait_for details_saved 30"
sleep 0.4
xdotool key Escape
check "Escape in Today then hides it" "wait_for '[ -z \"\$(today_visible)\" ]' 30"
check "focus returned to the editor after details and Today" "wait_for '[ \"\$(active)\" = \"$EDITOR_WIN\" ]' 30"

xdotool key ctrl+alt+g; wait_for '[ -n "$(today_visible)" ]' 50; sleep 0.4
xdotool key ctrl+alt+g
check "the shortcut toggles the glance closed again" "wait_for '[ -z \"\$(today_visible)\" ]' 30"
check "editor focused after the toggle" "wait_for '[ \"\$(active)\" = \"$EDITOR_WIN\" ]' 30"
xdotool key ctrl+shift+space; wait_for '[ -n "$(capture_visible)" ]' 50; sleep 0.4
CAP2="$(capture_visible)"
eval "$(xdotool getwindowgeometry --shell "$CAP2")"
CX=$((X + WIDTH/2)); CY=$((Y + HEIGHT/2))
check "capture still opens centred after the today window" "[ $((CX-720)) -ge -3 ] && [ $((CX-720)) -le 3 ] && [ $((CY-450)) -ge -30 ] && [ $((CY-450)) -le 30 ]"
xdotool key Escape; wait_for '[ -z "$(capture_visible)" ]' 30
echo "Later"
later_count(){ sql 'SELECT count(*) FROM notes WHERE later_at IS NOT NULL AND deleted_at IS NULL'; }
xdotool windowactivate --sync "$EDITOR_WIN"; sleep 0.3
xdotool key ctrl+alt+g; wait_for '[ -n "$(today_visible)" ]' 50; sleep 0.5
xdotool key l
check "L in Today parks the selected note in Later" "wait_for '[ \"\$(later_count)\" = 1 ]' 30"
check "the parked note is an open one" "[ \"\$(sql \"SELECT status FROM notes WHERE later_at IS NOT NULL\")\" = inbox ]"
check "Today stays open and focused after L" "[ -n \"\$(today_visible)\" ] && [ \"\$(active)\" = \"\$(today_visible)\" ]"
sleep 0.4; shot 08-today-after-later
xdotool key Escape; wait_for '[ -z "$(today_visible)" ]' 30

echo "Day view"
done_count(){ sql "SELECT count(*) FROM notes WHERE status = 'completed' AND deleted_at IS NULL"; }
# Make the oldest open note two days old, so it is carried with a tag.
sql "UPDATE notes SET created_at = created_at - 172800000 WHERE id = (SELECT id FROM notes WHERE status = 'inbox' AND later_at IS NULL AND deleted_at IS NULL ORDER BY created_at LIMIT 1)"
DONE_BEFORE="$(done_count)"
xdotool windowactivate --sync "$EDITOR_WIN"; sleep 0.3
xdotool key ctrl+alt+g; wait_for '[ -n "$(today_visible)" ]' 50; sleep 0.5
shot 10-today-origin
xdotool key Left; sleep 0.5
shot 11-today-yesterday
xdotool key x; sleep 0.5
check "X on a past day changes nothing" "[ \"\$(done_count)\" = \"$DONE_BEFORE\" ]"
# A day before any note: the empty state, for a look at its spacing.
xdotool key Left Left Left Left Left Left; sleep 0.8
shot 12-today-empty-day
xdotool key t; sleep 0.5
xdotool key x
check "T returns to today, where X works again" "wait_for '[ \"\$(done_count)\" != \"$DONE_BEFORE\" ]' 30"
xdotool key x
check "X again undoes it" "wait_for '[ \"\$(done_count)\" = \"$DONE_BEFORE\" ]' 30"
xdotool key Left; sleep 0.3; xdotool key t; sleep 0.3
xdotool key Escape
check "Esc still closes Today after browsing days" "wait_for '[ -z \"\$(today_visible)\" ]' 30"
NOTES_BEFORE_CRASH="$(sql 'SELECT count(*) FROM notes')"

echo "Crash and restart"
xdotool key ctrl+shift+space; wait_for '[ -n "$(capture_visible)" ]' 50; sleep 0.4
xdotool type --delay 15 "typed just before a crash"; sleep 0.6
# Roomy windows from the next start (settings are read at launch).
sql "INSERT OR REPLACE INTO settings (key, value) VALUES ('settings', '{\"window_size\":\"large\"}')"
kill -9 $APP; sleep 0.5
"$BIN" >"$WORK/app2.log" 2>&1 & APP=$!
check "app restarts after being killed" "wait_for 'xdotool search --onlyvisible --name \"^Posthaste\$\" >/dev/null 2>&1' 150"
check "all notes survived the crash" "[ \"\$(sql 'SELECT count(*) FROM notes')\" = \"$NOTES_BEFORE_CRASH\" ]"
check "the unsaved text survived as a draft" "[ \"\$(sql 'SELECT content FROM drafts')\" = 'typed just before a crash' ]"
sleep 1.5; shot 04-inbox

echo "Second launch"
"$BIN" --capture >/dev/null 2>&1; 
check "a second launch exits instead of running twice" "[ \$(pgrep -fc \"^$BIN\") = 1 ]"
check "\"--capture\" on a second launch opens capture in the running app" "wait_for '[ -n \"\$(capture_visible)\" ]' 50"
sleep 0.5; shot 05-draft-after-crash
xdotool key Escape; sleep 0.3

echo "Trash"
trashed(){ sql 'SELECT count(*) FROM notes WHERE deleted_at IS NOT NULL'; }
all_notes(){ sql 'SELECT count(*) FROM notes'; }
INBOX_WIN="$(for w in $(xdotool search --onlyvisible --name '^Posthaste$' 2>/dev/null); do
  eval "$(xdotool getwindowgeometry --shell "$w")"; echo "$HEIGHT $w"; done | sort -rn | head -1 | cut -d' ' -f2)"
xdotool windowactivate --sync "$INBOX_WIN"; sleep 0.5
TRASHED_BEFORE="$(trashed)"; ALL_BEFORE="$(all_notes)"
xdotool key 1; sleep 0.4; xdotool key j; sleep 0.2; xdotool key Delete
check "Delete in the inbox moves the note to the trash" "wait_for '[ \"\$(trashed)\" -gt \"$TRASHED_BEFORE\" ]' 30"
xdotool key 5; sleep 0.6
shot 13-trash
xdotool key j; sleep 0.2; xdotool key r
check "R in Trash restores it" "wait_for '[ \"\$(trashed)\" = \"$TRASHED_BEFORE\" ]' 30"
xdotool key 1; sleep 0.4; xdotool key j; sleep 0.2; xdotool key Delete; sleep 0.4
xdotool key 5; sleep 0.6; xdotool key j; sleep 0.2; xdotool key Delete; sleep 0.5
shot 14-trash-confirm
xdotool key Tab Return
check "Delete forever in Trash erases the note" "wait_for '[ \"\$(all_notes)\" -lt \"$ALL_BEFORE\" ]' 30"
check "nothing else was erased" "[ \"\$(all_notes)\" = \"\$(( $ALL_BEFORE - 1 ))\" ] && [ \"\$(trashed)\" = \"$TRASHED_BEFORE\" ]"
xdotool key 1; sleep 0.3

echo "Window size (Roomy)"
near(){ [ $(( $1 - $2 )) -ge -2 ] && [ $(( $1 - $2 )) -le 2 ]; }
xdotool windowactivate --sync "$EDITOR_WIN"; sleep 0.3
# The inbox window has the same title as capture: take the shortest one.
xdotool key ctrl+shift+space; wait_for '[ -n "$(capture_visible)" ]' 50; sleep 0.4
eval "$(for w in $(xdotool search --onlyvisible --name '^Posthaste$' 2>/dev/null); do
  eval "$(xdotool getwindowgeometry --shell "$w")"; echo "$HEIGHT $w"; done | sort -n | head -1 |
  { read -r _ w; xdotool getwindowgeometry --shell "$w"; })"
echo "        (capture ${WIDTH}x${HEIGHT})"
check "capture is 1.2 times as tall" "near $HEIGHT 322"
check "capture width follows the grown bounds (520 x 1.2)" "near $WIDTH 624"
sleep 0.3; shot 09-capture-roomy
xdotool key Escape; wait_for '[ -z "$(capture_visible)" ]' 30
xdotool key ctrl+alt+g; wait_for '[ -n "$(today_visible)" ]' 50; sleep 0.4
eval "$(xdotool getwindowgeometry --shell "$(today_visible)")"
echo "        (today ${WIDTH}x${HEIGHT})"
check "Today is 1.2 times as big" "near $WIDTH 528 && near $HEIGHT 648"
sleep 0.3; shot 10-today-roomy
xdotool key Escape; wait_for '[ -z "$(today_visible)" ]' 30

echo
echo "$PASS passed, $FAIL failed"
[ "$FAIL" -ne 0 ] && { echo "--- app log"; tail -20 "$WORK/app.log" "$WORK/app2.log" 2>/dev/null; }
exit "$FAIL"
