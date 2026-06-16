# breadpad / breadman

A quick-capture scratchpad and structured note viewer for Hyprland / Wayland, with AI-powered classification, reminders, recurrence, and snooze.

Two entry points, one binary, one shared workspace:

| Binary | Purpose |
|--------|---------|
| `breadpad` | Layer-shell capture popup — type a note, press Enter, done |
| `breadman` | Full note viewer and manager |

---

## Workspace layout

```
breadpad-shared   shared types, storage, classification, scheduler
breadpad          GTK4 layer-shell capture popup
breadman          GTK4 note viewer / manager
```

---

## Features

### Capture (`breadpad`)

- Layer-shell popup, centered, keyboard-exclusive — appears instantly on your keybind
- Single text field; press **Enter** or click **✓** to save, **Escape** to dismiss
- Optional manual type override before saving (defaults to AI classification)
- Timestamp and active Hyprland workspace recorded automatically

### Classification

Every note passes through a three-tier pipeline at capture time:

1. **Rule-based parser** — always runs first; handles time extraction ("at 7pm", "in 30 minutes", "tomorrow morning", "next Friday"), recurrence ("every Sunday at 9pm", "every weekday morning"), and strong type signals ("?" → question, "idea:" prefix → idea, action verbs → todo). High-confidence results skip the remaining tiers entirely.
2. **Small local ONNX model** — runs when Tier 1 can't confidently assign a type. Responsible for type classification only; Tier 1's extracted time, recurrence rule, and cleaned body are always preserved.
3. **Large local model via Ollama** — runs only when Tier 2 confidence falls below a configurable threshold. Communicates with a locally running Ollama instance over HTTP. If Ollama is unreachable, the Tier 2 result is used. No cloud APIs are involved.

Manual override always available — the AI-assigned type is shown as a chip you can tap to change before saving.

### Note types (built-in)

| Type | Example |
|------|---------|
| `todo` | "buy milk on the way home" |
| `reminder` | "pack calculator in bag at 7pm" |
| `idea` | "what if breadman had a calendar view" |
| `note` | "meeting went well, follow up Friday" |
| `question` | "why does nmcli drop on suspend?" |

User-defined tags can be added freely on top of the built-in types.

### Reminders, recurrence, and snooze

- **One-off reminders** — natural language time ("at 7pm", "in 30 minutes", "tomorrow morning") parsed at classification time; scheduled via a systemd user timer
- **Recurring reminders** — "every Sunday at 9pm", "every weekday morning" — stored as an iCal-compatible RRULE and re-scheduled on each trigger
- **Snooze** — notification popup includes snooze actions: 15 min / 1 hour / tomorrow morning / custom; snoozing reschedules the timer without touching the original note
- **Missed reminders** — if the system was off or suspended at the scheduled time, the reminder fires on next login

### Viewer (`breadman`)

- Sidebar with one entry per type + "All" and "Upcoming"
- Each note card shows: body, type chip, timestamp, workspace tag, recurrence badge if set
- **Upcoming** view: chronological list of all pending reminders and todos with times
- Inline editing — click any card to edit body, type, time, or recurrence
- Mark todo/reminder as done; done items move to an archive accessible via a toggle
- Search across all notes (full-text, instant)
- Sort: newest first (default)

### Theming

- Reads `~/.cache/wal/colors.json` (pywal) on startup — matches the rest of the bread ecosystem
- Falls back to Catppuccin Mocha
- CSS override: `~/.config/breadpad/style.css`
- `SIGHUP` reloads theme at runtime

---

## Storage

Notes are stored as JSONL at `~/.local/share/breadpad/notes.jsonl` — one JSON object per line, human-readable, easy to back up or script against.

```jsonl
{"id":"a1b2c3","body":"Pack calculator in bag","type":"reminder","time":"2026-05-25T19:00:00Z","rrule":null,"done":false,"workspace":"1","created":"2026-05-25T18:45:00Z","snoozed_until":null,"completed":null,"tags":[],"caldav_uid":null}
{"id":"d4e5f6","body":"Look into relm4 reactive patterns","type":"idea","time":null,"rrule":null,"done":false,"workspace":"2","created":"2026-05-25T14:10:00Z","snoozed_until":null,"completed":null,"tags":[],"caldav_uid":null}
```

Completed notes are never deleted — they gain `"done": true` and a `"completed"` timestamp. A separate `~/.local/share/breadpad/archive.jsonl` is written periodically for notes older than 30 days.

---

## AI classification

### Three-tier pipeline

#### Tier 1 — Rule-based parser

Always runs. Handles:
- **Time extraction**: "at 7pm", "in 30 minutes", "tomorrow morning", "next Friday at 9am"
- **Recurrence**: "every Sunday at 9pm", "every weekday morning" → stored as RRULE
- **Type signals**: leading "?" or "why/how/what" → `question`; "idea:" prefix or "what if" → `idea`; action verbs → `todo`; time present → `reminder`

Returns a calibrated confidence. If ≥ 0.82, Tiers 2 and 3 are skipped.

#### Tier 2 — Small local ONNX model

Runs when Tier 1 confidence is below threshold. Responsible for **type classification only** — Tier 1's extracted time, recurrence rule, and cleaned body are always preserved.

Invoked via `ort` (ONNX Runtime Rust bindings, `load-dynamic`) on the CPU. Requires an external `libonnxruntime.so`; set `model.ort_dylib_path` in `breadpad.toml` or let breadpad auto-discover it via `ORT_DYLIB_PATH`.

#### Tier 3 — Large local model via Ollama

Runs only when Tier 2 confidence falls below `model.ollama.confidence_threshold` (default 0.6). Sends a structured prompt to a locally running Ollama instance over HTTP and parses the JSON response for `type`, `body`, and `confidence`. The Ollama model runs on the iGPU via Ollama's own backend — breadpad does not manage GPU allocation for this tier.

If Ollama is unreachable or returns an invalid response, breadpad logs a warning and uses the Tier 2 result. No cloud APIs are used anywhere.

### Model location (Tier 2)

```
~/.local/share/breadpad/model/classifier.onnx
~/.local/share/breadpad/model/tokenizer.json
```

breadpad ships without a bundled model. Drop a compatible ONNX classifier and `tokenizer.json` at those paths, then configure `model.ort_dylib_path` to point at your ONNX Runtime library.

```bash
breadpad model-info   # shows active EP and model path
```

---

## Requirements

- Linux with a running Hyprland compositor
- GTK4 (≥ 4.12) + `gtk4-layer-shell`
- D-Bus session bus (for notifications)
- systemd user session (for timer-backed reminders)
- Rust 1.80+
- **Tier 2 (ONNX classifier):** An external `libonnxruntime.so`. Set `model.ort_dylib_path` in `breadpad.toml`, or set `ORT_DYLIB_PATH` in your environment. Without a library, Tier 2 is disabled; Tier 1 + 3 still work.
- **Tier 3 only (optional):** [Ollama](https://ollama.com) running locally with your chosen model pulled (`ollama pull llama3.2:3b`). Tier 3 is silently skipped if Ollama is not running.

---

## Installation

```bash
git clone https://github.com/breadway/breadpad
cd breadpad
cargo build --release
cp target/release/breadpad ~/.local/bin/
cp target/release/breadman ~/.local/bin/

# Place your ONNX classifier and tokenizer in the model directory
mkdir -p ~/.local/share/breadpad/model
# Then set model.ort_dylib_path in breadpad.toml to your libonnxruntime.so
```

On Arch Linux, install GTK4 dependencies first:

```bash
sudo pacman -S gtk4 gtk4-layer-shell
```

---

## Configuration

On first run, breadpad writes `~/.config/breadpad/breadpad.toml`:

```toml
[settings]
default_type = "note"          # fallback type if classification is skipped
workspace_tag = true           # tag notes with active Hyprland workspace
snooze_options = ["15m", "1h", "tomorrow_morning"]  # shown in notification actions
archive_after_days = 30

[model]
path = "~/.local/share/breadpad/model/classifier.onnx"
tokenizer = "~/.local/share/breadpad/model/tokenizer.json"
ort_dylib_path = ""              # optional: explicit path to libonnxruntime.so; auto-discovered when empty

[model.ollama]
endpoint = "http://localhost:11434"
model = "llama3.2:3b"          # any model you have pulled in Ollama
confidence_threshold = 0.6     # Tier 2 scores below this trigger Tier 3
enabled = true                 # set false to never call Ollama

[reminders]
default_morning = "08:00"      # what "tomorrow morning" resolves to
missed_grace_minutes = 60      # how long after boot to still fire a missed reminder

[calendar]
enabled = false                # turn on CalDAV sync (see below)
url = ""                       # CalDAV calendar collection URL
username = ""
password = ""                  # app password / token recommended
```

### Calendar sync (CalDAV)

When `[calendar].enabled = true`, reminders and dated notes are pushed to a
CalDAV calendar as events (tracked by `caldav_uid` on each note), so they show
up alongside the rest of your calendar.

1. Find your calendar's **collection URL**. It's the per-calendar CalDAV path,
   not the server root — e.g. Nextcloud:
   `https://host/remote.php/dav/calendars/<user>/<calendar-id>/`.
2. Create an **app password** for breadpad (don't use your main password):
   Nextcloud → Settings → Security → *Devices & sessions* → "Create new app
   password". Most CalDAV servers have an equivalent.
3. Fill in `breadpad.toml` (or BOS Settings → breadpad → Calendar):

   ```toml
   [calendar]
   enabled  = true
   url      = "https://host/remote.php/dav/calendars/me/breadpad/"
   username = "me"
   password = "xxxx-xxxx-xxxx-xxxx"
   ```
4. Restart breadpad. New dated/reminder notes sync up; the `caldav_uid` field
   links each note to its event so updates and deletes stay in step.

If the server is unreachable, breadpad logs a warning and keeps the note
locally — sync is best-effort and never blocks capture.

---

## Usage

### breadpad (capture)

```bash
# Open the capture popup (bind this to a key in hyprland.conf)
breadpad

# Open with a pre-selected type
breadpad --type todo

# Skip AI classification (save as plain note)
breadpad --no-classify

# Show model and storage status
breadpad --status
```

Hyprland keybind:

```
bind = $mainMod, N, exec, breadpad
```

### breadman (viewer)

```bash
# Open the note viewer
breadman

# Open directly to a specific type view
breadman --view todo
breadman --view upcoming

# Mark a note done by ID (scriptable)
breadman done <id>

# List upcoming reminders in the terminal
breadman upcoming --plain
```

---

## Scheduler

breadpad manages reminders via systemd user timers. Each scheduled note gets a transient timer unit:

```
breadpad-reminder-<id>.timer
breadpad-reminder-<id>.service
```

The service unit runs `breadpad fire <id>`, which sends a `notify-send` notification with snooze actions. Snoozing writes the new time back to the note and creates a replacement timer. Recurring notes create the next timer immediately on fire.

You can inspect pending timers:

```bash
systemctl --user list-timers 'breadpad-*'
```

---

## Testing

`breadpad-test` is a CLI test harness for the classification pipeline. It runs a JSON corpus of labelled inputs through any tier of the pipeline and reports pass/fail.

```bash
# Run Tier 1 (rule-based only) — fast, no model needed
breadpad-test run

# See only failing cases
breadpad-test run --format failures

# Run Tier 2 (+ ONNX model)
breadpad-test run --tier 2

# Run full pipeline including Ollama
breadpad-test run --tier all

# Machine-readable output
breadpad-test run --format json
```

### Corpus format

Default path: `breadpad-test/corpus.json`. Override with `--corpus <path>`.

```json
[
  {
    "input": "pack my calculator in my bag tonight",
    "expected_type": "todo",
    "expected_time": null,
    "expected_body": "pack my calculator in my bag",
    "expected_rrule": null,
    "notes": "no time specified, should not infer one"
  }
]
```

- `expected_time` — `HH:MM`; date component is ignored so tests are never date-sensitive
- `expected_rrule` — matched as substring of the actual RRULE string
- Any `null` field is skipped — only non-null fields are asserted

### Tier modes

| `--tier` | What runs |
|----------|-----------|
| `1` (default) | Tier 1 rule-based parser only — no model required |
| `2` | Tiers 1 + 2 (ONNX classifier) |
| `3` / `all` | Full pipeline including Tier 3 Ollama |

### Corpus management

```bash
# Interactively add an entry
breadpad-test add

# Show entry #5 and the pipeline's actual output
breadpad-test show 5

# Open corpus file in $EDITOR at entry #5
breadpad-test edit 5
```

### Typical tuning workflow

```bash
# 1. See what Tier 1 gets wrong
breadpad-test run --tier 1 --format failures

# 2. Edit parser.rs, then rerun
cargo build -p breadpad-shared && breadpad-test run --tier 1 --format failures

# 3. Once Tier 1 is stable, audit Tier 2 regressions
breadpad-test run --tier 2 --format failures
```

---

## Nextcloud Calendar integration

breadpad can push scheduled notes and recurring reminders to a CalDAV calendar (Nextcloud or any RFC 4791-compliant server). No cloud APIs are used — everything goes directly to your own server over HTTPS.

### What gets pushed

Notes with a scheduled time (`time` field) or recurrence rule (`rrule`) are pushed as VEVENT entries when saved. Notes without a time are not pushed. Deleting a note also deletes the corresponding calendar event.

### Configuration

Add a `[calendar]` section to `~/.config/breadpad/breadpad.toml`:

```toml
[calendar]
enabled = true
url = "https://nextcloud.example.com/remote.php/dav/calendars/you/breadpad/"
username = "you"
password = "app-password-here"   # use a Nextcloud app password, not your login password
```

The calendar must already exist on the server. Create it in the Nextcloud Calendar app before enabling this integration.

### CLI commands

```bash
# Verify the CalDAV connection and credentials
breadpad calendar test

# List CalDAV UIDs for all scheduled notes (queries the server if enabled, local store if not)
breadpad calendar list-uid

# Show the CalDAV UID for a specific note by its local ID
breadpad calendar list-uid <note-id>
```

### Event format

Each note is pushed as a VEVENT with:
- `UID` — `<note-id>@breadpad` (stable and deterministic)
- `SUMMARY` — note body
- `DTSTART` / `DTEND` — scheduled time (or creation time for recurring notes without a fixed start)
- `RRULE` — recurrence rule if set
- `DESCRIPTION` — `type=<note-type>`

### Security note

Store your CalDAV password using a Nextcloud app password rather than your account password. App passwords can be revoked individually from the Nextcloud security settings.

---

## Module layout

| Crate / module | Responsibility |
|----------------|---------------|
| `breadpad-shared/src/types.rs` | `Note`, `NoteType`, `RecurrenceRule`, `SnoozeState` |
| `breadpad-shared/src/store.rs` | JSONL read/write, atomic saves, archive rotation |
| `breadpad-shared/src/classifier.rs` | Three-tier pipeline orchestration (Tier 1 → 2 → 3) |
| `breadpad-shared/src/parser.rs` | Tier 1: rule-based time/recurrence/type parsing |
| `breadpad-shared/src/ai.rs` | Tier 3: Ollama HTTP client, prompt construction, response parsing |
| `breadpad-shared/src/calendar.rs` | CalDAV client: push, delete, list events; iCal VEVENT builder |
| `breadpad-shared/src/scheduler.rs` | systemd timer creation, snooze, recurrence next-occurrence |
| `breadpad/src/main.rs` | GTK4 layer-shell popup, text field, type chip selector |
| `breadman/src/main.rs` | GTK4 app entry, sidebar, note list, search |
| `breadman/src/views/` | `upcoming.rs`, `archive.rs`, per-type list views |
| `breadman/src/editor.rs` | Inline note editor popover |

---

## License

MIT
