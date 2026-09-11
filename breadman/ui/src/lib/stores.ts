import { writable, derived } from "svelte/store";
import type { Config, ErrorEntry, Note } from "./types";
import { BUILTIN_TYPES } from "./types";
import { api } from "./api";
import { effectiveTime } from "./format";

export const notes = writable<Note[]>([]);
export const archive = writable<Note[]>([]);
export const errors = writable<ErrorEntry[]>([]);
export const config = writable<Config | null>(null);

export const activeView = writable<string>("all");
export const search = writable<string>("");

export interface EditorState {
  open: boolean;
  /** null = New note; a Note = Edit note. */
  note: Note | null;
}
export const editor = writable<EditorState>({ open: false, note: null });

export function openNewNote() {
  editor.set({ open: true, note: null });
}
export function openEditNote(note: Note) {
  editor.set({ open: true, note });
}
export function closeEditor() {
  editor.set({ open: false, note: null });
}

export async function refreshNotes() {
  notes.set(await api.listNotes());
}
export async function refreshArchive() {
  archive.set(await api.listArchive());
}
export async function refreshErrors() {
  errors.set(await api.listErrors());
}
export async function refreshConfig() {
  config.set(await api.getConfig());
}

export async function bootstrap() {
  try {
    activeView.set(await api.initialView());
  } catch {
    /* keep default */
  }
  await Promise.allSettled([refreshNotes(), refreshArchive(), refreshConfig(), refreshErrors()]);
}

/** Sidebar badge counts. Active (not-done) per type + view totals. */
export const counts = derived<[typeof notes, typeof archive], Record<string, number>>(
  [notes, archive],
  ([$notes, $archive]) => {
    const active = $notes.filter((n) => !n.done);
    const out: Record<string, number> = {};
    for (const t of BUILTIN_TYPES) out[t] = 0;
    for (const n of active) {
      if (n.type in out) out[n.type]++;
    }
    out.all = active.length;
    out.upcoming = active.filter(
      (n) => (n.type === "reminder" || n.type === "todo") && effectiveTime(n) !== null,
    ).length;
    out.archive = $archive.length + $notes.filter((n) => n.done).length;
    return out;
  },
);
