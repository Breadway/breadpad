import { invoke } from "@tauri-apps/api/core";
import type { Config, ClassifyHint, ErrorEntry, Note, NoteDraft } from "./types";

export const api = {
  initialView: () => invoke<string>("get_initial_view"),
  listNotes: () => invoke<Note[]>("list_notes"),
  listArchive: () => invoke<Note[]>("list_archive"),
  getConfig: () => invoke<Config>("get_config"),
  saveConfig: (cfg: Config) => invoke<void>("save_config", { cfg }),
  listErrors: () => invoke<ErrorEntry[]>("list_errors"),
  classifyPreview: (body: string) => invoke<ClassifyHint>("classify_preview", { body }),
  createNote: (draft: NoteDraft) => invoke<Note>("create_note", { draft }),
  updateNote: (id: string, draft: NoteDraft) => invoke<Note>("update_note", { id, draft }),
  setDone: (id: string, done: boolean) => invoke<Note>("set_done", { id, done }),
  deleteNote: (id: string) => invoke<void>("delete_note", { id }),
  themeCss: () => invoke<string>("get_theme_css"),
};
