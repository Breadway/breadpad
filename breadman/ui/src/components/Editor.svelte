<script lang="ts">
  import { editor, closeEditor, refreshNotes, refreshArchive, refreshErrors } from "../lib/stores";
  import { api } from "../lib/api";
  import type { ClassifyHint, Note } from "../lib/types";
  import { BUILTIN_TYPES } from "../lib/types";

  interface Props {
    note: Note | null;
  }
  let { note }: Props = $props();

  const isEdit = note !== null;

  function toLocalStamp(iso: string | null): string {
    if (!iso) return "";
    const d = new Date(iso);
    const p = (n: number) => String(n).padStart(2, "0");
    return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
  }

  let body = $state(note?.body ?? "");
  let timeText = $state(toLocalStamp(note?.time ?? null));
  let rrule = $state(note?.rrule ?? "");
  let picked = $state<string>(note?.type ?? "");
  let hint = $state<ClassifyHint | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);

  const TIME_HELP = "tomorrow 9am  /  at 7pm  /  2026-08-01 09:00";
  const effectiveType = $derived(picked || hint?.note_type || "note");

  // Live Tier-1 classification preview, debounced on the body text.
  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const text = body;
    clearTimeout(timer);
    if (!text.trim()) {
      hint = null;
      return;
    }
    timer = setTimeout(async () => {
      try {
        hint = await api.classifyPreview(text);
      } catch {
        hint = null;
      }
    }, 180);
  });

  async function submit() {
    error = null;
    busy = true;
    const draft = { body, note_type: picked, time_text: timeText, rrule };
    try {
      if (isEdit && note) {
        await api.updateNote(note.id, draft);
      } else {
        await api.createNote(draft);
      }
      await Promise.allSettled([refreshNotes(), refreshArchive(), refreshErrors()]);
      closeEditor();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function remove() {
    if (!note) return;
    busy = true;
    try {
      await api.deleteNote(note.id);
      await Promise.allSettled([refreshNotes(), refreshArchive()]);
      closeEditor();
    } finally {
      busy = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") closeEditor();
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) void submit();
  }
</script>

<svelte:window onkeydown={onKey} />

<div
  class="scrim"
  role="presentation"
  onclick={(e) => {
    if (e.target === e.currentTarget) closeEditor();
  }}
>
  <div class="dialog" role="dialog" aria-modal="true" aria-label={isEdit ? "Edit note" : "New note"}>
    <header>{isEdit ? "Edit note" : "New note"}</header>
    <div class="fields">
      <div class="field">
        <label for="ed-body">Note</label>
        <!-- svelte-ignore a11y_autofocus -->
        <textarea id="ed-body" rows="2" bind:value={body} autofocus placeholder="Jot something down…"></textarea>
      </div>

      <div class="field">
        <label for="ed-time">Time</label>
        <input id="ed-time" class="mono" type="text" bind:value={timeText} placeholder={TIME_HELP} />
        <div class="hint">{TIME_HELP}</div>
      </div>

      <div class="field">
        <label for="ed-rep">Repeat</label>
        <input id="ed-rep" class="mono" type="text" bind:value={rrule} placeholder="RRULE:FREQ=WEEKLY;BYDAY=MO" />
      </div>

      <div class="field">
        <span class="label" id="ed-type-label" style="font-size:12px;color:var(--ink-dim)">Type</span>
        <div class="type-picker" role="group" aria-labelledby="ed-type-label" style="margin-top:5px">
          {#each BUILTIN_TYPES as t (t)}
            <button
              class="pick {t}"
              class:on={effectiveType === t}
              onclick={() => (picked = picked === t ? "" : t)}
              aria-pressed={effectiveType === t}>{t}</button
            >
          {/each}
        </div>
      </div>

      {#if hint}
        <div class="classified">
          {picked ? "set to" : "auto-classified"}
          <b>{effectiveType}</b>
          &middot; tier 1 {hint.tier}
          <span class="conf-bar"><i style="width:{Math.round(hint.confidence * 100)}%"></i></span>
          {hint.confidence.toFixed(2)}
        </div>
      {/if}

      {#if error}<div class="err">{error}</div>{/if}
    </div>

    <footer>
      {#if isEdit}
        <button class="btn ghost-danger" onclick={remove} disabled={busy}>Delete</button>
      {:else}
        <span></span>
      {/if}
      <div class="right">
        <button class="btn" onclick={closeEditor} disabled={busy}>Cancel</button>
        <button class="btn primary" onclick={submit} disabled={busy || !body.trim()}>
          {busy ? "Saving…" : isEdit ? "Save note" : "Add note"}
        </button>
      </div>
    </footer>
  </div>
</div>
