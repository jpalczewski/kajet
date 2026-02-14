<script lang="ts">
  import type { ActionEvent } from '$lib/types';
  import { t } from '$lib/stores/i18n.svelte';
  let { queries }: { queries: ActionEvent[] } = $props();

  let expandedIndex = $state<number | null>(null);

  function toggleExpand(index: number) {
    expandedIndex = expandedIndex === index ? null : index;
  }

  function formatTime(ts: string): string {
    const d = new Date(ts);
    const ms = d.getTime() % 1000;
    return d.toLocaleTimeString('en-GB', { hour12: false }) +
      '.' + String(ms).padStart(3, '0');
  }

</script>

{#if queries.length === 0}
  <div class="empty">{t('query_log_empty', 'No queries yet')}</div>
{:else}
  {#each queries as event, index}
    {#if event.type === 'QueryExecuted'}
      {@const data = event.data}
      <div class="query-item">
        <button
          class="query-header"
          onclick={() => toggleExpand(index)}
          aria-expanded={expandedIndex === index}
        >
          <span class="time">{formatTime(data.timestamp)}</span>
          <span class="query-text">{data.query}</span>
          <span class="result-count">{data.results.length} {t('query_results', 'results')}</span>
          <span class="duration">{data.duration_ms}ms</span>
          <span class="expand-icon">{expandedIndex === index ? '▼' : '▶'}</span>
        </button>

        {#if expandedIndex === index}
          <div class="results-panel">
            {#if data.results.length === 0}
              <div class="no-results">{t('dashboard_no_results', 'No results')}</div>
            {:else}
              {#each data.results as result}
                <a
                  class="result-item"
                  href="/documents/{encodeURIComponent(result.note_path)}"
                >
                  <div class="result-header">
                    <span class="result-breadcrumb">{result.breadcrumb}</span>
                    <span class="result-score">{result.score.toFixed(4)}</span>
                  </div>
                  <div class="result-path">{result.note_path}</div>
                  <div class="result-preview">{result.content_preview}</div>
                </a>
              {/each}
            {/if}
          </div>
        {/if}
      </div>
    {/if}
  {/each}
{/if}

<style>
  .empty {
    color: #555;
    font-size: 0.8rem;
    padding: 2rem;
    text-align: center;
  }

  .query-item {
    margin-bottom: 0.5rem;
  }

  .query-header {
    width: 100%;
    display: flex;
    gap: 0.5rem;
    align-items: center;
    padding: 0.5rem 0.6rem;
    background: #161616;
    border: 1px solid #222;
    border-radius: 6px;
    font-size: 0.78rem;
    cursor: pointer;
    transition: background 0.15s;
    font-family: inherit;
    text-align: left;
  }

  .query-header:hover {
    background: #1a1a1a;
  }

  .query-header:focus-visible {
    outline: 2px solid #7af;
    outline-offset: 2px;
  }

  .time {
    color: #555;
    flex-shrink: 0;
    font-size: 0.75rem;
  }

  .query-text {
    color: #7af;
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .result-count {
    color: #5a5;
    flex-shrink: 0;
  }

  .duration {
    color: #888;
    font-size: 0.7rem;
    flex-shrink: 0;
  }

  .expand-icon {
    color: #666;
    font-size: 0.7rem;
    flex-shrink: 0;
    margin-left: 0.3rem;
  }

  .results-panel {
    margin-top: 0.3rem;
    padding: 0.5rem;
    background: #0f0f0f;
    border: 1px solid #1a1a1a;
    border-radius: 6px;
  }

  .no-results {
    color: #666;
    font-size: 0.75rem;
    padding: 0.5rem;
    text-align: center;
  }

  .result-item {
    display: block;
    text-decoration: none;
    color: inherit;
    width: 100%;
    padding: 0.5rem 0.6rem;
    margin: 0.25rem 0;
    background: #161616;
    border: 1px solid #222;
    border-left: 3px solid #333;
    border-radius: 4px;
    cursor: pointer;
    transition: all 0.15s;
    font-family: inherit;
    text-align: left;
  }

  .result-item:hover {
    background: #1a1a1a;
    border-left-color: #7af;
  }

  .result-item:focus-visible {
    outline: 2px solid #7af;
    outline-offset: 2px;
  }

  .result-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.25rem;
  }

  .result-breadcrumb {
    color: #777;
    font-size: 0.7rem;
  }

  .result-score {
    color: #5a5;
    font-size: 0.7rem;
    font-weight: 600;
  }

  .result-path {
    color: #888;
    font-size: 0.7rem;
    margin-bottom: 0.3rem;
  }

  .result-preview {
    color: #bbb;
    font-size: 0.75rem;
    line-height: 1.5;
    max-height: 3rem;
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }
</style>
