<script lang="ts">
  import NoteRow from "./NoteRow.svelte";
  import Icon from "./Icon.svelte";
  import { notes, search } from "../lib/stores";
  import { fmtRowDate } from "../lib/format";

  interface Props {
    /** "all" or a specific note type. */
    view: string;
    emptyText: string;
  }
  let { view, emptyText }: Props = $props();

  const filtered = $derived.by(() => {
    const q = $search.trim().toLowerCase();
    return $notes
      .filter((n) => !n.done)
      .filter((n) => view === "all" || n.type === view)
      .filter((n) => q === "" || n.body.toLowerCase().includes(q))
      .sort((a, b) => b.created.localeCompare(a.created));
  });
</script>

{#if filtered.length === 0}
  <div class="empty">
    <Icon name="list" size={28} />
    <span>{$search.trim() ? `No notes match "${$search.trim()}".` : emptyText}</span>
  </div>
{:else}
  <div class="list">
    {#each filtered as note (note.id)}
      <NoteRow {note} dateLabel={fmtRowDate(note.created)} showType={view === "all"} />
    {/each}
  </div>
{/if}
