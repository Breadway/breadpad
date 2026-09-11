<script lang="ts">
  import Sidebar from "./components/Sidebar.svelte";
  import NoteList from "./components/NoteList.svelte";
  import Upcoming from "./components/Upcoming.svelte";
  import Archive from "./components/Archive.svelte";
  import Settings from "./components/Settings.svelte";
  import Errors from "./components/Errors.svelte";
  import Editor from "./components/Editor.svelte";
  import Icon from "./components/Icon.svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { invoke } from "@tauri-apps/api/core";
  import { get } from "svelte/store";
  import { tick } from "svelte";
  import {
    activeView,
    search,
    editor,
    counts,
    notes,
    bootstrap,
    openEditNote,
    openNewNote,
  } from "./lib/stores";

  void bootstrap().then(async () => {
    // `breadman --view editor` opens straight into the editor (first note, or
    // a new one). Kept as a screenshot / deep-link hook.
    if (get(activeView) === "editor") {
      const first = get(notes)[0] ?? null;
      activeView.set("all");
      if (first) openEditNote(first);
      else openNewNote();
    }

    // `--screenshot` mode is the only listener (see `notify_ui_ready` in
    // commands.rs); everywhere else this is a harmless no-op. `tick()` waits
    // for the store updates above to flush to the DOM, then a rAF round
    // trip waits for the browser to actually paint that DOM before telling
    // the backend it's safe to grab the frame.
    await tick();
    await new Promise((resolve) => requestAnimationFrame(resolve));
    void invoke("notify_ui_ready").catch(() => {});
  });

  const appWindow = getCurrentWindow();
  function drag(e: MouseEvent) {
    if (e.button === 0) void appWindow.startDragging();
  }

  const TITLES: Record<string, string> = {
    all: "All notes",
    upcoming: "Upcoming",
    todo: "Todo",
    reminder: "Reminder",
    idea: "Idea",
    note: "Note",
    question: "Question",
    archive: "Archive",
    settings: "Settings",
    errors: "Errors",
  };

  const isList = $derived(!["settings", "errors"].includes($activeView));
  const emptyText = $derived(
    {
      all: "No notes yet. Jot something down to get started.",
      todo: "No todos yet.",
      reminder: "No reminders yet.",
      idea: "No ideas captured yet.",
      note: "No notes yet.",
      question: "No open questions yet.",
    }[$activeView] ?? "Nothing here yet.",
  );

  const countLabel = $derived.by(() => {
    const c = $counts;
    switch ($activeView) {
      case "all":
        return `${c.all} active`;
      case "upcoming":
        return `${c.upcoming} pending`;
      case "archive":
        return `${c.archive} archived`;
      case "settings":
      case "errors":
        return "";
      default:
        return `${c[$activeView] ?? 0}`;
    }
  });
</script>

<div class="window">
  <div class="titlebar" onmousedown={drag} role="presentation">
    <span class="mark"><Icon name="loaf" size={13} /></span>
    <span class="wordmark">breadman <i>&middot; manager</i></span>
    <span class="grow"></span>
    <button class="win-btn" aria-label="Minimize" onclick={() => appWindow.minimize()}>
      <svg width="12" height="12" viewBox="0 0 12 12"><line x1="2" y1="6" x2="10" y2="6" stroke="currentColor" stroke-width="1.4" /></svg>
    </button>
    <button class="win-btn close" aria-label="Close" onclick={() => appWindow.close()}>
      <svg width="12" height="12" viewBox="0 0 12 12"><path d="M2 2l8 8M10 2l-8 8" stroke="currentColor" stroke-width="1.4" /></svg>
    </button>
  </div>

  <div class="body">
    <Sidebar />

    <main class="main">
      {#if isList}
        <div class="toolbar">
          <div class="toolbar-inner">
            <h1>
              {TITLES[$activeView] ?? $activeView}
              {#if countLabel}<b>{countLabel}</b>{/if}
            </h1>
            <span class="grow"></span>
            <label class="search">
              <Icon name="search" size={14} />
              <input type="text" placeholder="Search all notes" bind:value={$search} />
            </label>
          </div>
        </div>
      {/if}

      {#if $activeView === "upcoming"}
        <div class="scroll"><Upcoming /></div>
      {:else if $activeView === "archive"}
        <div class="scroll"><Archive /></div>
      {:else if $activeView === "settings"}
        <Settings />
      {:else if $activeView === "errors"}
        <Errors />
      {:else}
        <div class="scroll"><NoteList view={$activeView} {emptyText} /></div>
      {/if}
    </main>
  </div>
</div>

{#if $editor.open}
  <Editor note={$editor.note} />
{/if}
