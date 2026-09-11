const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const DAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

const pad = (n: number) => String(n).padStart(2, "0");
const hm = (d: Date) => `${pad(d.getHours())}:${pad(d.getMinutes())}`;

function startOfDay(d: Date): number {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
}

/** Creation stamp for the All / per-type rows: "Sep 10 14:30". */
export function fmtRowDate(iso: string): string {
  const d = new Date(iso);
  return `${MONTHS[d.getMonth()]} ${pad(d.getDate())} ${hm(d)}`;
}

/** Effective-time stamp for Upcoming: "Today 07:00" / "Wed 09:00" /
 *  "Aug 01 09:00", widening the label the further out the date is. */
export function fmtUpcoming(iso: string): string {
  const d = new Date(iso);
  const now = new Date();
  const days = Math.round((startOfDay(d) - startOfDay(now)) / 86_400_000);
  if (days === 0) return `Today ${hm(d)}`;
  if (days === 1) return `Tomorrow ${hm(d)}`;
  if (days > 1 && days < 7) return `${DAYS[d.getDay()]} ${hm(d)}`;
  return `${MONTHS[d.getMonth()]} ${pad(d.getDate())} ${hm(d)}`;
}

/** Completion stamp for Archive: "done Sep 04". */
export function fmtArchive(iso: string | null): string {
  if (!iso) return "done";
  const d = new Date(iso);
  return `done ${MONTHS[d.getMonth()]} ${pad(d.getDate())}`;
}

/** True when the instant is today or already past. Drives the row's
 *  amber "due" tint. */
export function isDue(iso: string | null): boolean {
  if (!iso) return false;
  return new Date(iso).getTime() <= startOfDay(new Date()) + 86_400_000 - 1;
}

export function effectiveTime(note: { time: string | null; snoozed_until: string | null }): string | null {
  return note.snoozed_until ?? note.time;
}
