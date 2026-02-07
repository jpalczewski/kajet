<script lang="ts">
  import type { QueryEvent } from '../lib/types';
  import { t } from '../lib/stores/i18n.svelte';

  let { event }: { event: QueryEvent } = $props();

  const time = $derived(new Date(event.timestamp).toLocaleTimeString());
</script>

<div class="event">
  <span class="time">{time}</span>
  <span class="query">{event.query}</span>
  <span class="count">&rarr; {event.num_results} {t('results_suffix', 'results')}</span>
</div>

<style>
  .event {
    padding: 0.3rem 0;
    border-bottom: 1px solid #1a1a1a;
    font-size: 0.78rem;
    display: flex;
    gap: 0.5rem;
  }
  .time { color: #555; flex-shrink: 0; }
  .query { color: #7af; }
  .count { color: #5a5; }
</style>
