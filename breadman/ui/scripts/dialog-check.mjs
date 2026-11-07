import assert from "node:assert/strict";
import { createServer } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { render } from "svelte/server";

const server = await createServer({
  root: process.cwd(),
  configFile: false,
  logLevel: "error",
  plugins: [svelte({ compilerOptions: { dev: false } })],
  server: { middlewareMode: true },
  appType: "custom",
});

const { default: Editor } = await server.ssrLoadModule("/src/components/Editor.svelte");

const base = {
  id: "note-1",
  body: "team sync",
  type: "reminder",
  time: "2030-01-15T09:00:00Z",
  rrule: null,
  done: false,
  workspace: null,
  created: "2030-01-01T00:00:00Z",
  snoozed_until: null,
  completed: null,
  tags: [],
  caldav_uid: null,
};

function html(props) {
  return render(Editor, { props }).body;
}

// New note: friendly repeat options, no raw RRULE input, generated time example.
const fresh = html({ note: null });
for (const label of [
  "Does not repeat",
  "Daily",
  "Weekly",
  "Monthly",
  "Weekdays (Mon to Fri)",
  "Custom rule",
]) {
  assert.ok(fresh.includes(label), `missing repeat option: ${label}`);
}
assert.ok(fresh.includes("tomorrow morning"), "time hint lost its relative example");
assert.match(fresh, /\d{4}-\d{2}-\d{2} 09:00/, "time hint should include a generated date");
assert.ok(fresh.includes('value="none" selected'), "new notes should default to no repeat");
assert.ok(!fresh.includes('id="ed-rep-rule"'), "raw rule input should stay hidden for presets");
console.log("PASS new note: friendly options, generated time example, no raw RRULE field");

// Stored weekly RRULE maps back to the friendly Weekly choice, no raw input.
const weekly = html({
  note: { ...base, rrule: "RRULE:FREQ=WEEKLY;BYDAY=TU;BYHOUR=10;BYMINUTE=30;BYSECOND=0" },
});
assert.ok(weekly.includes('value="weekly" selected'), "weekly rule should select Weekly");
assert.ok(!weekly.includes('id="ed-rep-rule"'), "recognised weekly rule should hide raw input");
console.log("PASS weekly note: stored RRULE maps to the Weekly option");

// A rule with extra parts stays Custom and keeps the raw escape hatch.
const customRule = "RRULE:FREQ=WEEKLY;INTERVAL=2;BYDAY=MO;BYHOUR=9;BYMINUTE=0;BYSECOND=0";
const custom = html({ note: { ...base, rrule: customRule } });
assert.ok(custom.includes('value="custom" selected'), "non-preset rule should select Custom");
assert.ok(custom.includes('id="ed-rep-rule"'), "custom rule should show the raw input");
assert.ok(custom.includes(customRule), "custom rule text should be preserved verbatim");
console.log("PASS custom rule: raw escape hatch preserved");

await server.close();
