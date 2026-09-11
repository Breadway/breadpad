// Mirrors breadpad-shared's serde types and breadman's command DTOs.

export const BUILTIN_TYPES = ["todo", "reminder", "idea", "note", "question"] as const;
export type BuiltinType = (typeof BUILTIN_TYPES)[number];
export type NoteType = BuiltinType | (string & {});

export interface Note {
  id: string;
  body: string;
  type: NoteType;
  time: string | null;
  rrule: string | null;
  done: boolean;
  workspace: string | null;
  created: string;
  snoozed_until: string | null;
  completed: string | null;
  tags: string[];
  caldav_uid: string | null;
}

export interface OllamaConfig {
  endpoint: string;
  model: string;
  confidence_threshold: number;
  enabled: boolean;
}

export interface Config {
  settings: {
    default_type: string;
    workspace_tag: boolean;
    snooze_options: string[];
    archive_after_days: number;
  };
  model: {
    path: string;
    tokenizer: string;
    ort_dylib_path: string;
    ollama: OllamaConfig;
  };
  reminders: {
    default_morning: string;
    missed_grace_minutes: number;
  };
  calendar: {
    enabled: boolean;
    url: string;
    username: string;
    password: string;
  };
}

export interface ClassifyHint {
  note_type: string;
  time: string | null;
  rrule: string | null;
  confidence: number;
  tier: string;
}

export interface ErrorEntry {
  time: string;
  message: string;
}

export interface NoteDraft {
  body: string;
  note_type: string;
  time_text: string;
  rrule: string;
}

export function isBuiltin(t: string): t is BuiltinType {
  return (BUILTIN_TYPES as readonly string[]).includes(t);
}

/** The class suffix used across chips / dots / accent-bars. */
export function typeClass(t: NoteType): string {
  return isBuiltin(t) ? t : "tag";
}
