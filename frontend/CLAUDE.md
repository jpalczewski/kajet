# Frontend Guide

## Stack

- **Svelte 5** (runes mode) + SvelteKit + TypeScript
- **Deno** for package management (not npm/pnpm)
- **Vite** for build
- **Static adapter** - built to `dist/`, embedded into Rust binary via rust-embed

## Build & Dev

```bash
cd frontend
deno install             # Install dependencies (creates node_modules/)
deno task dev            # Dev server (http://localhost:5173)
deno task build          # Production build → dist/ (REQUIRED before cargo build)
deno task check          # Type checking (svelte-check)
deno fmt                 # Format code
```

**Critical:** Always run `deno task build` before `cargo build --release`. The Rust binary embeds the built frontend via rust-embed.

## Svelte 5 Runes Patterns

Project uses **Svelte 5 runes** (not old `$:` syntax):

| Rune | Purpose | Example |
|------|---------|---------|
| `$state()` | Reactive local state | `let count = $state(0)` |
| `$derived()` | Computed values | `let doubled = $derived(count * 2)` |
| `$props()` | Component props (typed) | `let { result }: { result: SearchResult } = $props()` |
| `$effect()` | Side effects | `$effect(() => { console.log(count) })` |

**Example component:**

```svelte
<script lang="ts">
  import type { SearchResult } from '$lib/types';

  let { result }: { result: SearchResult } = $props();
  let expanded = $state(false);
  let displayText = $derived(expanded ? result.content : result.content_preview);

  $effect(() => {
    if (expanded) {
      console.log('Expanded result:', result.note_path);
    }
  });
</script>

<button onclick={() => expanded = !expanded}>
  {displayText}
</button>
```

## Type Safety

**All API types auto-generated from Rust via ts-rs:**

- Location: `src/lib/types/generated/*.ts`
- Generated on: `cargo build` (backend compilation)
- **Never edit manually** - changes will be overwritten

Import from generated types:

```typescript
import type { ActionEvent, SearchResult, DocumentDetail } from '$lib/types';
```

## WebSocket Store Pattern

**Svelte 5 store with auto-reconnect** (`src/lib/stores/websocket.svelte.ts`):

```typescript
let connected = $state(false);
let ws: WebSocket | null = null;

function connect() {
  const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
  ws = new WebSocket(`${protocol}//${location.host}/ws`);

  ws.onopen = () => { connected = true };
  ws.onclose = () => {
    connected = false;
    setTimeout(connect, 2000);  // Auto-reconnect after 2s
  };

  ws.onmessage = (e) => {
    const event: ActionEvent = JSON.parse(e.data);
    // Handle events...
  };
}
```

**Usage in components:**

```svelte
<script lang="ts">
  import { getWsStore } from '$lib/stores/websocket.svelte';

  const ws = getWsStore();
  // Access: ws.logs, ws.queries, ws.connected, etc.
</script>
```

## Component Conventions

- **File naming:** PascalCase (e.g., `SearchResultItem.svelte`)
- **Props:** Always typed with `$props()` destructuring
- **State:** Prefer `$state()` over exported `let` for reactivity
- **Events:** Use inline `onclick={...}` with arrow functions (not `on:click`)
- **Styling:** Scoped `<style>` blocks, no global CSS

## Gotchas

- **Deno, not npm:** Use `deno install`, not `npm install`
- **Build before Rust compilation:** Frontend must be built first (`deno task build` → `cargo build`)
- **Svelte 5 is new:** Old tutorials using `$:` reactive declarations won't work - use `$derived()` instead
- **TypeScript strict mode:** All types must be explicit, no implicit `any`
- **SvelteKit static adapter:** No SSR at runtime - everything prerendered at build time
