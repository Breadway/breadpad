<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { Note } from "../lib/types";
  import { typeClass } from "../lib/types";
  import { api } from "../lib/api";
  import { openEditNote, refreshNotes, refreshArchive, refreshErrors } from "../lib/stores";

  interface Props {
    note: Note;
    dateLabel: string;
    due?: boolean;
    showType?: boolean;
    showDone?: boolean;
  }
  let { note, dateLabel, due = false, showType = true, showDone = true }: Props = $props();

  async function reload() {
    await Promise.allSettled([refreshNotes(), refreshArchive(), refreshErrors()]);
  }
  async function toggleDone() {
    await api.setDone(note.id, !note.done);
    await reload();
  }
  async function remove() {
    await api.deleteNote(note.id);
    await reload();
  }
  function edit() {
    openEditNote(note);
  }
</script>

<div
  class="row"
  class:done={note.done}
  role="button"
  tabindex="0"
  onclick={edit}
  onkeydown={(e) => (e.key === "Enter" || e.key === " ") && edit()}
>
  <span class="time" class:due>{dateLabel}</span>
  <span class="body">{note.body}</span>
  {#if note.rrule}<span class="rrule" title={note.rrule}>&#x21bb;</span>{/if}
  {#if note.workspace}<span class="ws">ws:{note.workspace}</span>{/if}
  {#if showType}<span class="chip {typeClass(note.type)}">{note.type}</span>{/if}
  <span class="actions">
    {#if showDone}
      <button
        class="act done"
        title={note.done ? "Mark not done" : "Mark done"}
        onclick={(e) => {
          e.stopPropagation();
          void toggleDone();
        }}
      >
        <Icon name="check" size={13} fill />
      </button>
    {/if}
    <button
      class="act"
      title="Edit"
      onclick={(e) => {
        e.stopPropagation();
        edit();
      }}
    >
      <Icon name="pencil" size={13} />
    </button>
    <button
      class="act danger"
      title="Delete"
      onclick={(e) => {
        e.stopPropagation();
        void remove();
      }}
    >
      <Icon name="trash" size={13} />
    </button>
  </span>
</div>
