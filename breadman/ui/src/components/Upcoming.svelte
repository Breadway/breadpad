<script lang="ts">
  import NoteRow from "./NoteRow.svelte";
  import Icon from "./Icon.svelte";
  import type { Note } from "../lib/types";
  import { notes, search } from "../lib/stores";
  import { fmtUpcoming, isDue, effectiveTime } from "../lib/format";

  const rows = $derived.by(() => {
    const q = $search.trim().toLowerCase();
    const out: Array<{ n: Note; t: string }> = [];
    for (const n of $notes) {
      if (n.done || (n.type !== "reminder" && n.type !== "todo")) continue;
      const t = effectiveTime(n);
      if (t === null) continue;
      if (q !== "" && !n.body.toLowerCase().includes(q)) continue;
      out.push({ n, t });
    }
    out.sort((a, b) => a.t.localeCompare(b.t));
    return out;
  });
</script>

{#if rows.length === 0}
  <div class="empty">
    <Icon name="calendar" size={28} />
    <span>No upcoming reminders or todos.</span>
  </div>
{:else}
  <div class="list">
    {#each rows as { n, t } (n.id)}
      <NoteRow note={n} dateLabel={fmtUpcoming(t)} due={isDue(t)} />
    {/each}
  </div>
{/if}
