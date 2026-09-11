import { listen } from "@tauri-apps/api/event";
import { api } from "./api";

function apply(css: string) {
  let el = document.getElementById("bread-theme") as HTMLStyleElement | null;
  if (!el) {
    el = document.createElement("style");
    el.id = "bread-theme";
    // Insert first so app.css (bundled later in <head>) still wins on
    // component rules while these :root vars feed into it.
    document.head.prepend(el);
  }
  el.textContent = css;
}

/** Pull the initial palette CSS and subscribe to live `bread-theme reload`
 *  / wallpaper changes, mirroring the GTK build's live reload. */
export async function initTheme(): Promise<void> {
  try {
    apply(await api.themeCss());
  } catch (e) {
    console.error("theme: initial load failed", e);
  }
  try {
    await listen<string>("theme://changed", (ev) => apply(ev.payload));
  } catch (e) {
    console.error("theme: subscribe failed", e);
  }
}
