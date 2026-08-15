# breadpad — bread event integration

breadpad is a standalone capture popup: it works exactly the same with or
without `breadd` running. When breadd *is* present, the `breadpad` binary
publishes events into the shared bread automation fabric after actions that
already happened. See the parent `bread` repo's `Documentation.md` —
specifically its "Namespaces" and "Integrating a bread\* app" sections — for
the general convention this follows.

App id: **`pad`**. Transport: `bread-utils`'s `bread_client` module
(feature `bread-client`) — the capture popup links it directly. breadpad is
short-lived (one popup, or one `fire <id>` invocation from the existing
systemd user timer), so each `emit` is its own short-lived connection.
There is no long-running breadpad daemon and therefore no command
subscription.

`breadman` (the viewer) does not emit or subscribe. Notes created or edited
there are not quick-capture, and it is not on the reminder-fire path.

## Events published (`bread.pad.*`)

| Event | Data | When |
|-------|------|------|
| `bread.pad.captured` | `{ "id": "<note id>" }` | The capture popup saved a note successfully (`Store::save_note` returned `Ok`). Not emitted when the field is empty, the window is dismissed, classification-only preview happens, or the write fails. |
| `bread.pad.reminder.due` | `{ "id": "<note id>" }` | `breadpad fire <id>` decided the reminder is due (`Scheduler::fire` returned true) and is about to show the reminder window. This is the existing in-process systemd-timer hook (`breadpad-reminder-<id>.timer` → `breadpad fire <id>`), not a new daemon. Not emitted when the note is missing, the fire is outside the missed-grace window, or the reminder window is opened as a `--screenshot` sample. |

Note bodies are never included in the payload — only the local note id.
Notes stay in `~/.local/share/breadpad/notes.jsonl`; the event bus is for
*notifications about* capture and due reminders, not a channel for note
content.

## Commands honored (`bread.command.pad.*`)

None. breadpad has no persistent process that could subscribe, and the
actions a command verb would map to already exist as local CLI / keybind
paths (`breadpad` for capture, `breadpad fire <id>` for the reminder
window, `breadman` for viewing and editing). Stubbing `capture` / `snooze`
/ `done` on the bus without a subscriber (or inventing a daemon just to
hold one) would be a no-op dressed up as an API. If breadpad later grows a
long-running piece that can honor a verb for real, add the verb then.

## Fail-safe behavior

- If breadd isn't installed or isn't running, `emit` is a silent no-op
  (`BreadClient::emit` never blocks or errors the caller). Capture, save,
  systemd timers, and the reminder window are entirely unaffected.
- There is no command subscription to reconnect. Restarting breadd does
  not require restarting breadpad; the next real capture or fire will emit
  again if breadd is reachable at that moment.
