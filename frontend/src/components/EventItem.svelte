<script lang="ts">
  import type { ActionEvent } from '../lib/types';
  import { t } from '../lib/stores/i18n.svelte';

  let { event }: { event: ActionEvent } = $props();

  // Only QueryExecuted events should be passed to this component
  const queryData = $derived(
    event.type === 'QueryExecuted' ? event.data : null
  );

  const time = $derived(
    queryData ? new Date(queryData.timestamp).toLocaleTimeString() : ''
  );
</script>

{#if queryData}
  <div class="event">
    <span class="time">{time}</span>
    <span class="query">{queryData.query}</span>
    <span class="count">&rarr; {queryData.results.length} {t('results_suffix', 'results')}</span>
    <span class="duration">{queryData.duration_ms}ms</span>
  </div>
{/if}

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
  .duration { color: #888; font-size: 0.7rem; margin-left: auto; }
</style>
