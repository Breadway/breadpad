<script lang="ts">
  import Icon from "./Icon.svelte";
  import { activeView, counts, openNewNote, search } from "../lib/stores";
  import { BUILTIN_TYPES } from "../lib/types";

  function go(view: string) {
    search.set("");
    activeView.set(view);
  }

  const typeLabels: Record<string, string> = {
    todo: "Todo",
    reminder: "Reminder",
    idea: "Idea",
    note: "Note",
    question: "Question",
  };
</script>

<nav class="sidebar">
  <button class="new-note" onclick={openNewNote}>
    <Icon name="plus" size={15} />
    New note
  </button>

  <div class="side-label">Views</div>
  <button class="nav-item" class:active={$activeView === "all"} onclick={() => go("all")}>
    <span class="ico"><Icon name="list" /></span>
    All
    <span class="count">{$counts.all}</span>
  </button>
  <button class="nav-item" class:active={$activeView === "upcoming"} onclick={() => go("upcoming")}>
    <span class="ico"><Icon name="calendar" /></span>
    Upcoming
    <span class="count">{$counts.upcoming}</span>
  </button>

  <div class="side-label">Types</div>
  {#each BUILTIN_TYPES as t (t)}
    <button class="nav-item" class:active={$activeView === t} onclick={() => go(t)}>
      <span class="ico"><span class="dot type-dot {t}"></span></span>
      {typeLabels[t]}
      <span class="count">{$counts[t] ?? 0}</span>
    </button>
  {/each}

  <div class="side-label">More</div>
  <button class="nav-item" class:active={$activeView === "archive"} onclick={() => go("archive")}>
    <span class="ico"><Icon name="folder" /></span>
    Archive
    <span class="count">{$counts.archive}</span>
  </button>
  <button class="nav-item" class:active={$activeView === "settings"} onclick={() => go("settings")}>
    <span class="ico"><Icon name="gear" /></span>
    Settings
  </button>
  <button class="nav-item" class:active={$activeView === "errors"} onclick={() => go("errors")}>
    <span class="ico"><Icon name="warning" /></span>
    Errors
  </button>
</nav>
