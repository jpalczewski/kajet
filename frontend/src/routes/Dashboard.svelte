<script lang="ts">
  import { getWsStore } from '../lib/stores/websocket.svelte';
  import { t } from '../lib/stores/i18n.svelte';
  import { searchVault, getStatus } from '../lib/api';
  import type { SearchResult } from '../lib/types';
  import EventItem from '../components/EventItem.svelte';
  import SearchResultItem from '../components/SearchResultItem.svelte';

  const ws = getWsStore();

  let query = $state('');
  let results = $state<SearchResult[]>([]);
  let searching = $state(false);
  let error = $state('');
  let debounceTimer: ReturnType<typeof setTimeout>;

  function onInput(e: Event) {
    const value = (e.target as HTMLInputElement).value;
    query = value;
    clearTimeout(debounceTimer);
    if (!value.trim()) {
      results = [];
      error = '';
      return;
    }
    debounceTimer = setTimeout(() => doSearch(value), 300);
  }

  async function doSearch(q: string) {
    searching = true;
    error = '';
    try {
      results = await searchVault(q);
    } catch (e) {
      error = String(e);
      results = [];
    } finally {
      searching = false;
    }
  }
</script>

<div class="grid">
  <div class="panel">
    <h2>{t('panel_query_log', 'mcp query log')}</h2>
    {#if ws.queries.length === 0}
      <div class="empty">{t('waiting_for_queries', 'Waiting for queries from Claude...')}</div>
    {:else}
      {#each ws.queries as event}
        <EventItem {event} />
      {/each}
    {/if}
  </div>

  <div class="panel">
    <h2>{t('panel_stats', 'stats')}</h2>
    {#if ws.isIndexing && ws.indexProgress}
      <div class="stats-item">
        <span class="stats-label">{t('indexing_status', 'Indexing')}</span>
        <span class="stats-value">{ws.indexProgress.processed} / {ws.indexProgress.total}</span>
      </div>
    {:else if ws.stats}
      <div class="stats-item">
        <span class="stats-label">{t('stats_notes', 'Notes')}</span>
        <span class="stats-value">{ws.stats.note_count}</span>
      </div>
      <div class="stats-item">
        <span class="stats-label">{t('stats_chunks', 'Chunks')}</span>
        <span class="stats-value">{ws.stats.chunk_count}</span>
      </div>
    {:else}
      <div class="empty">{t('stats_message', 'Dashboard running. Search below or use via MCP.')}</div>
    {/if}
  </div>

  <div class="panel search-panel">
    <h2>{t('panel_search', 'search playground')}</h2>
    <div class="search-box">
      <input
        type="text"
        placeholder={t('search_placeholder', 'search your vault...')}
        value={query}
        oninput={onInput}
      />
      {#if searching}
        <span class="spinner">{t('searching', 'searching...')}</span>
      {/if}
    </div>
    <div class="results">
      {#if error}
        <div class="error">{error}</div>
      {:else if query && !searching && results.length === 0}
        <div class="empty">
          {#if ws.isIndexing}
            {t('indexing_in_progress', 'Indexing in progress...')}
          {:else}
            {t('dashboard_no_results', 'No results')}
          {/if}
        </div>
      {:else}
        {#each results as result}
          <SearchResultItem {result} />
        {/each}
      {/if}
    </div>
  </div>
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
  }
  @media (max-width: 900px) { .grid { grid-template-columns: 1fr; } }

  .panel {
    background: #111;
    border: 1px solid #222;
    border-radius: 8px;
    padding: 1rem;
    max-height: 70vh;
    overflow-y: auto;
  }
  .panel h2 {
    color: #666;
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-bottom: 0.75rem;
  }

  .search-panel { grid-column: span 2; }
  @media (max-width: 900px) { .search-panel { grid-column: span 1; } }

  .search-box { position: relative; }

  input[type="text"] {
    background: #1a1a1a;
    border: 1px solid #333;
    color: #fff;
    padding: 0.6rem 0.8rem;
    width: 100%;
    border-radius: 6px;
    font-family: inherit;
    font-size: 0.85rem;
    outline: none;
  }
  input[type="text"]:focus { border-color: #555; }
  input[type="text"]::placeholder { color: #444; }

  .spinner { color: #555; font-size: 0.75rem; }
  .empty { color: #555; font-size: 0.8rem; padding: 1rem; text-align: center; }
  .error { color: #d55; font-size: 0.8rem; padding: 0.5rem; }
  .results { margin-top: 0.5rem; }

  .stats-item {
    display: flex;
    justify-content: space-between;
    padding: 0.5rem 0;
    border-bottom: 1px solid #1a1a1a;
    font-size: 0.85rem;
  }
  .stats-label { color: #888; }
  .stats-value { color: #7af; font-weight: 600; }
</style>
