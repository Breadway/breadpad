<script lang="ts">
  import { config, refreshConfig } from "../lib/stores";
  import { api } from "../lib/api";
  import type { Config } from "../lib/types";
  import { BUILTIN_TYPES } from "../lib/types";

  // Local working copy; committed on Save.
  let draft = $state<Config | null>(null);
  let baseline = $state("");
  let saving = $state(false);

  $effect(() => {
    if ($config && draft === null) {
      draft = structuredClone($config);
      baseline = JSON.stringify(draft);
    }
  });

  const dirty = $derived(draft !== null && JSON.stringify(draft) !== baseline);

  async function save() {
    if (!draft) return;
    saving = true;
    try {
      await api.saveConfig(draft);
      await refreshConfig();
      baseline = JSON.stringify(draft);
    } finally {
      saving = false;
    }
  }

  const TYPE_HELP: Record<string, string> = {
    todo: "an action to tick off",
    reminder: "something with a time that pings you",
    idea: "a thought to keep",
    note: "a plain note",
    question: "an open question",
  };

  // Snooze shortcuts shown on the reminder popup, stored as short tokens.
  const SNOOZE_PRESETS = [
    { token: "15m", label: "15 min" },
    { token: "30m", label: "30 min" },
    { token: "1h", label: "1 hour" },
    { token: "3h", label: "3 hours" },
    { token: "tomorrow_morning", label: "Tomorrow morning" },
  ];
  function toggleSnooze(token: string) {
    if (!draft) return;
    const cur = draft.settings.snooze_options;
    draft.settings.snooze_options = cur.includes(token)
      ? cur.filter((t) => t !== token)
      : [...cur, token];
  }
  const customSnooze = $derived(
    (draft?.settings.snooze_options ?? []).filter(
      (t) => !SNOOZE_PRESETS.some((p) => p.token === t),
    ),
  );

  // "Missed grace" in plain terms: how late a reminder may still fire after
  // the machine was off/asleep at its time.
  const CATCHUP = [
    { v: 0, label: "Don't fire missed reminders" },
    { v: 15, label: "If under 15 minutes late" },
    { v: 30, label: "If under 30 minutes late" },
    { v: 60, label: "If under 1 hour late" },
    { v: 120, label: "If under 2 hours late" },
    { v: 1440, label: "Fire them whenever I'm back" },
  ];
  function setCatchup(v: string) {
    if (draft) draft.reminders.missed_grace_minutes = Number(v);
  }

  function setThreshold(v: string) {
    if (draft) draft.model.ollama.confidence_threshold = Number(v);
  }
</script>

{#if draft}
  <div class="form-view">
    <div class="settings">
      <h1>Settings</h1>
      <p class="lede">Shared by breadpad and breadman. Changes apply to both.</p>

      <!-- ---------- Notes ---------- -->
      <div class="set-group">
        <h2>Notes</h2>
        <div class="boxed-list">
          <div class="set-row">
            <span class="rl">
              <b>New notes default to</b>
              <span>Used when breadpad can't tell the type from what you typed. Currently:
                {TYPE_HELP[draft.settings.default_type] ?? "a plain note"}.</span>
            </span>
            <div class="type-picker">
              {#each BUILTIN_TYPES as t (t)}
                <button
                  class="pick {t}"
                  class:on={draft.settings.default_type === t}
                  onclick={() => draft && (draft.settings.default_type = t)}>{t}</button
                >
              {/each}
            </div>
          </div>

          <div class="set-row">
            <span class="rl">
              <b>Tag notes with the workspace</b>
              <span>Record which Hyprland workspace you were on when you jotted it.</span>
            </span>
            <button
              class="toggle"
              class:on={draft.settings.workspace_tag}
              aria-label="Tag notes with the workspace"
              aria-pressed={draft.settings.workspace_tag}
              onclick={() => draft && (draft.settings.workspace_tag = !draft.settings.workspace_tag)}
            ></button>
          </div>

          <div class="set-row">
            <span class="rl">
              <b>Tidy done notes away after</b>
              <span>Completed notes leave the main list and move to the archive.</span>
            </span>
            <span class="ctl">
              <input
                class="field-input num"
                type="number"
                min="1"
                max="365"
                bind:value={draft.settings.archive_after_days}
              />
              <span class="unit">days</span>
            </span>
          </div>
        </div>
      </div>

      <!-- ---------- Reminders ---------- -->
      <div class="set-group">
        <h2>Reminders</h2>
        <div class="boxed-list">
          <div class="set-row">
            <span class="rl">
              <b>My morning starts at</b>
              <span>What "tomorrow morning" means when you snooze, and when daily reminders fire.</span>
            </span>
            <input class="field-input time" type="time" bind:value={draft.reminders.default_morning} />
          </div>

          <div class="set-row">
            <span class="rl">
              <b>Snooze shortcuts</b>
              <span>The quick buttons on a reminder pop-up.</span>
            </span>
            <div class="type-picker">
              {#each SNOOZE_PRESETS as p (p.token)}
                <button
                  class="pick"
                  class:on={draft.settings.snooze_options.includes(p.token)}
                  aria-pressed={draft.settings.snooze_options.includes(p.token)}
                  onclick={() => toggleSnooze(p.token)}>{p.label}</button
                >
              {/each}
              {#each customSnooze as t (t)}
                <button class="pick on" onclick={() => toggleSnooze(t)} title="Custom option, click to remove">{t}</button>
              {/each}
            </div>
          </div>

          <div class="set-row">
            <span class="rl">
              <b>If a reminder was missed</b>
              <span>When the machine was off or asleep at the time.</span>
            </span>
            <select
              class="field-input select"
              value={String(draft.reminders.missed_grace_minutes)}
              onchange={(e) => setCatchup(e.currentTarget.value)}
            >
              {#each CATCHUP as o (o.v)}
                <option value={String(o.v)}>{o.label}</option>
              {/each}
            </select>
          </div>
        </div>
      </div>

      <!-- ---------- Calendar ---------- -->
      <div class="set-group">
        <h2>Calendar sync</h2>
        <p>Copy dated notes and reminders into a calendar (Nextcloud or any CalDAV server). Off by default.</p>
        <div class="boxed-list">
          <div class="set-row">
            <span class="rl"><b>Sync to a calendar</b></span>
            <button
              class="toggle"
              class:on={draft.calendar.enabled}
              aria-label="Sync to a calendar"
              aria-pressed={draft.calendar.enabled}
              onclick={() => draft && (draft.calendar.enabled = !draft.calendar.enabled)}
            ></button>
          </div>
          {#if draft.calendar.enabled}
            <div class="set-row">
              <span class="rl">
                <b>Calendar address</b>
                <span>The CalDAV URL for one calendar, e.g.
                  https://cloud.example.com/remote.php/dav/calendars/you/personal/</span>
              </span>
              <input class="field-input text" type="text" bind:value={draft.calendar.url} />
            </div>
            <div class="set-row">
              <span class="rl"><b>Username</b></span>
              <input class="field-input text" type="text" bind:value={draft.calendar.username} />
            </div>
            <div class="set-row">
              <span class="rl">
                <b>Password</b>
                <span>Use an app password, not your main one. Stored in plain text in breadpad.toml.</span>
              </span>
              <input class="field-input text" type="password" bind:value={draft.calendar.password} />
            </div>
          {/if}
        </div>
      </div>

      <!-- ---------- Advanced ---------- -->
      <details class="advanced">
        <summary>Advanced: local AI models</summary>
        <p class="adv-note">
          breadpad sorts notes with plain rules first. These optional local models only run when the
          rules are unsure. Nothing is sent to the cloud.
        </p>

        <div class="set-group">
          <h2>Local AI model (Ollama)</h2>
          <div class="boxed-list">
            <div class="set-row">
              <span class="rl"><b>Use a local Ollama model</b></span>
              <button
                class="toggle"
                class:on={draft.model.ollama.enabled}
                aria-label="Use a local Ollama model"
                aria-pressed={draft.model.ollama.enabled}
                onclick={() => draft && (draft.model.ollama.enabled = !draft.model.ollama.enabled)}
              ></button>
            </div>
            {#if draft.model.ollama.enabled}
              <div class="set-row">
                <span class="rl"><b>Ollama address</b></span>
                <input class="field-input text" type="text" bind:value={draft.model.ollama.endpoint} />
              </div>
              <div class="set-row">
                <span class="rl"><b>Model name</b></span>
                <input class="field-input text" type="text" bind:value={draft.model.ollama.model} />
              </div>
              <div class="set-row">
                <span class="rl">
                  <b>Ask the model when the rules are less than
                    {Math.round(draft.model.ollama.confidence_threshold * 100)}% sure</b>
                </span>
                <span class="ctl range">
                  <input
                    type="range"
                    min="0"
                    max="1"
                    step="0.05"
                    value={draft.model.ollama.confidence_threshold}
                    oninput={(e) => setThreshold(e.currentTarget.value)}
                    aria-label="Confidence threshold"
                  />
                </span>
              </div>
            {/if}
          </div>
        </div>

        <div class="set-group">
          <h2>On-device classifier files</h2>
          <p>An optional ONNX model you supply yourself. Leave blank to skip. Shared with breadpad.</p>
          <div class="boxed-list">
            <div class="set-row">
              <span class="rl"><b>Model file</b></span>
              <input class="field-input text" type="text" bind:value={draft.model.path} />
            </div>
            <div class="set-row">
              <span class="rl"><b>Tokenizer file</b></span>
              <input class="field-input text" type="text" bind:value={draft.model.tokenizer} />
            </div>
            <div class="set-row">
              <span class="rl">
                <b>ONNX runtime library</b>
                <span>Path to libonnxruntime.so. Blank = find it automatically.</span>
              </span>
              <input class="field-input text" type="text" bind:value={draft.model.ort_dylib_path} />
            </div>
          </div>
        </div>
      </details>

      <div class="set-save">
        {#if dirty}<span>Unsaved changes</span>{/if}
        <button class="btn primary" disabled={!dirty || saving} onclick={save}>
          {saving ? "Saving…" : "Save settings"}
        </button>
      </div>
    </div>
  </div>
{/if}
