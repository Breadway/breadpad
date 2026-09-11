<script lang="ts">
  import NoteRow from "./NoteRow.svelte";
  import Icon from "./Icon.svelte";
  import type { Note } from "../lib/types";
  import { notes, archive, search } from "../lib/stores";
  import { fmtArchive } from "../lib/format";

  // Archive view = rotated-out archive.jsonl PLUS notes still in notes.jsonl
  // that are marked done (not yet rotated), matching the GTK build.
  const rows = $derived.by(() => {
    const q = $search.trim().toLowerCase();
    const seen = new Set<string>();
    const merged: Note[] = [];
    for (const n of [...$notes.filter((x) => x.done), ...$archive]) {
      if (seen.has(n.id)) continue;
      seen.add(n.id);
      if (q !== "" && !n.body.toLowerCase().includes(q)) continue;
      merged.push(n);
    }
    return merged.sort((a, b) =>
      (b.completed ?? b.created).localeCompare(a.completed ?? a.created),
    );
  });
</script>

{#if rows.length === 0}
  <div class="empty">
    <Icon name="folder" size={28} />
    <span>Nothing archived yet.</span>
  </div>
{:else}
  <div class="list">
    {#each rows as note (note.id)}
      <NoteRow {note} dateLabel={fmtArchive(note.completed)} showDone={false} />
    {/each}
  </div>
{/if}
