# breadpad — bread event integration

breadpad is a standalone capture popup: it works exactly the same with or
without `breadd` running. When breadd *is* present, the `breadpad` binary
publishes events into the shared bread automation fabric after actions that
already happened. See the parent `bread` repo's `Documentation.md` —
specifically its "Namespaces" and "Integrating a bread\* app" sections — for
the general convention this follows.

App id: **`pad`**. Transport: `bread-utils`'s `bread_client` module
(feature `bread-client`) — the capture popup links it directly. One-shot
popup / `fire <id>` invocations each `emit` on their own short-lived
connection. Command verbs are only received while `breadpad listen` is
running — that process holds the `bread.command.pad.**` subscription
open.

`breadman` (the viewer) does not emit or subscribe. Notes created or edited
there are not quick-capture, and it is not on the reminder-fire path.

## Events published (`bread.pad.*`)

| Event | Data | When |
|-------|------|------|
| `bread.pad.captured` | `{ "id": "<note id>" }` | The capture popup saved a note successfully (`Store::save_note` returned `Ok`). Not emitted when the field is empty, the window is dismissed, classification-only preview happens, or the write fails. |
| `bread.pad.reminder.due` | `{ "id": "<note id>" }` | `breadpad fire <id>` decided the reminder is due (`Scheduler::fire` returned true) and is about to show the reminder window. This is the existing in-process systemd-timer hook (`breadpad-reminder-<id>.timer` → `breadpad fire <id>`), not a new daemon. Not emitted when the note is missing, the fire is outside the missed-grace window, or the reminder window is opened as a `--screenshot` sample. |
| `bread.pad.capture.done` | `{}` | `bread.command.pad.capture` was received and `breadpad` was spawned. This is the command confirmation, not proof the popup mapped — the spawned process is the same no-args invocation as the capture keybind. |
| `bread.pad.capture.failed` | `{ "error": "<message>" }` | `bread.command.pad.capture` was received but this binary could not be started. |

Note bodies are never included in the payload — only the local note id.
Notes stay in `~/.local/share/breadpad/notes.jsonl`; the event bus is for
*notifications about* capture and due reminders, not a channel for note
content.

## Commands honored (`bread.command.pad.*`)

These are only received while `breadpad listen` is running. Publishing a
command with no subscriber is a silent no-op — that is the documented
bread convention, not a breadpad bug.

| Verb | Data | Effect |
|------|------|--------|
| `capture` | none | Same as running `breadpad` with no args: open the capture popup. Emits `bread.pad.capture.done` / `.failed`. |

```lua
bread.spawn(function()
    bread.emit("bread.command.pad.capture")
    bread.wait("bread.pad.capture.done", { timeout = 5000 })
end)
```

### Not implemented: extra verbs

There is no `snooze` / `done` / `fire` command verb. Reminder fire
already exists as `breadpad fire <id>` (systemd user timer), and
viewing/editing lives in `breadman`. If/when a bus verb maps to real
extra behavior, add it then — do not stub one as a no-op ahead of it.

## Fail-safe behavior

- If breadd isn't installed or isn't running, `emit` is a silent no-op
  (`BreadClient::emit` never blocks or errors the caller) and the
  command subscription simply never receives anything. Capture, save,
  systemd timers, and the reminder window are entirely unaffected.
- If breadd restarts, the command subscription reconnects automatically
  (`BreadClient::subscribe`'s background thread has its own backoff
  loop); no restart of `breadpad listen` is needed.
- If `breadpad listen` is not running, commands are a graceful no-op at
  the bus (no subscriber). One-shot capture / `fire` still emit
  `bread.pad.captured` / `bread.pad.reminder.due` on their own
  short-lived connection.
