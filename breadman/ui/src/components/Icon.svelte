<script lang="ts">
  // Single stroke-icon set, currentColor, 24-grid. Kept inline (CSP blocks
  // external assets and the set is tiny).
  interface Props {
    name: string;
    size?: number;
    fill?: boolean;
  }
  let { name, size = 16, fill = false }: Props = $props();

  const paths: Record<string, string> = {
    plus: "M12 5v14M5 12h14",
    search: "M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14ZM20 20l-3.5-3.5",
    list: "M4 6h16M4 12h16M4 18h16",
    calendar: "M4 5h16v16H4zM8 3v4M16 3v4M4 10h16",
    folder: "M3 8h18v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2ZM3 8 5 4h14l2 4",
    gear: "M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6ZM12 3v2M12 19v2M5 5l1.5 1.5M17.5 17.5 19 19M3 12h2M19 12h2M5 19l1.5-1.5M17.5 6.5 19 5",
    warning: "M12 3 2 20h20ZM12 9v5M12 17v.01",
    check: "M5 12l5 5L20 6",
    pencil: "M12 20h9M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z",
    trash: "M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13",
    chevron: "M6 9l6 6 6-6",
    loaf: "M4 13c0-3 2.5-5 8-5s8 2 8 5-1 6-2 6H6c-1 0-2-3-2-6ZM8 12v.01M12 12v.01M16 12v.01",
  };
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={fill ? 2.4 : 2}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <path d={paths[name] ?? ""} />
</svg>
