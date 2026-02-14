<script lang="ts">
  import { getWsStore } from '$lib/stores/websocket.svelte';
  import { t } from '$lib/stores/i18n.svelte';
  import type { ActionEvent } from '$lib/types';
  import QueryLog from './QueryLog.svelte';

  const ws = getWsStore();

  const LEVELS = ['TRACE', 'DEBUG', 'INFO', 'WARN', 'ERROR'];
  let minLevel = $state('INFO');
  let autoScroll = $state(true);
  let container: HTMLDivElement | undefined = $state();
  let activeTab = $state<'logs' | 'queries'>('logs');
  let expandedIndex = $state<number | null>(null);

  function levelIndex(level: string): number {
    return LEVELS.indexOf(level.toUpperCase());
  }

  const filtered = $derived(
    ws.logs.filter((e: ActionEvent) => {
      if (e.type !== 'LogEntry') return false;
      return levelIndex(e.data.level) >= levelIndex(minLevel);
    })
  );

  function formatTime(ts: string): string {
    const d = new Date(ts);
    return d.toLocaleTimeString('en-GB', { hour12: false }) +
      '.' + String(d.getMilliseconds()).padStart(3, '0');
  }

  function levelColor(level: string): string {
    switch (level.toUpperCase()) {
      case 'TRACE': return '#666';
      case 'DEBUG': return '#888';
      case 'INFO': return '#4a4';
      case 'WARN': return '#da4';
      case 'ERROR': return '#d44';
      default: return '#888';
    }
  }

  function toggleExpand(index: number) {
    expandedIndex = expandedIndex === index ? null : index;
  }

  function hasFields(entry: ActionEvent): boolean {
    return entry.type === 'LogEntry' &&
           entry.data.fields !== undefined &&
           Object.keys(entry.data.fields).length > 0;
  }


  function handleScroll() {
    if (!container) return;
    const { scrollTop, scrollHeight, clientHeight } = container;
    autoScroll = scrollHeight - scrollTop - clientHeight < 40;
  }

  $effect(() => {
    // Re-run when filtered changes
    filtered;
    if (autoScroll && container) {
      requestAnimationFrame(() => {
        if (container) container.scrollTop = container.scrollHeight;
      });
    }
  });
</script>

<div class="panel">
  <div class="header">
    <div class="tabs">
      <button
        class="tab"
        class:active={activeTab === 'logs'}
        onclick={() => activeTab = 'logs'}
      >
        {t('logs_tab_logs', 'Logs')}
      </button>
      <button
        class="tab"
        class:active={activeTab === 'queries'}
        onclick={() => activeTab = 'queries'}
      >
        {t('logs_tab_queries', 'Queries')}
      </button>
    </div>
    {#if activeTab === 'logs'}
      <div class="controls">
        <label>
          <span class="label">{t('logs_level_filter', 'Min level')}</span>
          <select bind:value={minLevel}>
            {#each LEVELS as level}
              <option value={level}>{level}</option>
            {/each}
          </select>
        </label>
        <span class="count">{filtered.length} {t('logs_entries', 'entries')}</span>
      </div>
    {:else}
      <span class="count">{ws.queries.length} {t('logs_entries', 'entries')}</span>
    {/if}
  </div>

  {#if activeTab === 'logs'}
    <div class="log-list" bind:this={container} onscroll={handleScroll}>
      {#if filtered.length === 0}
        <div class="empty">{t('logs_empty', 'No log entries')}</div>
      {:else}
        {#each filtered as entry, index}
          {#if entry.type === 'LogEntry'}
            {@const data = entry.data}
            {@const expandable = hasFields(entry)}
            <div class="log-item">
              <div
                class="log-entry"
                class:expandable
                class:expanded={expandedIndex === index}
                onclick={() => expandable && toggleExpand(index)}
                role={expandable ? 'button' : undefined}
                tabindex={expandable ? 0 : undefined}
              >
                <span class="time">{formatTime(data.timestamp)}</span>
                <span class="level" style:color={levelColor(data.level)}>{data.level.padEnd(5)}</span>
                <span class="target">{data.target}</span>
                <span class="message">{data.message}</span>
                {#if expandable}
                  <span class="expand-icon">{expandedIndex === index ? '▼' : '▶'}</span>
                {/if}
              </div>
              {#if expandable && expandedIndex === index && data.fields}
                <div class="fields-panel">
                  {#each Object.entries(data.fields) as [key, value]}
                    <div class="field-row">
                      <span class="field-key">{key}:</span>
                      <span class="field-value">{JSON.stringify(value)}</span>
                    </div>
                  {/each}
                </div>
              {/if}
            </div>
          {/if}
        {/each}
      {/if}
    </div>
  {:else}
    <div class="query-list">
      <QueryLog queries={ws.queries} />
    </div>
  {/if}
</div>

<style>
  .panel {
    background: #111;
    border: 1px solid #222;
    border-radius: 8px;
    padding: 1rem;
    display: flex;
    flex-direction: column;
    height: calc(100vh - 8rem);
  }
  .header {
    display: flex;
    align-items: center;
    gap: 1rem;
    margin-bottom: 0.75rem;
  }
  .tabs {
    display: flex;
    gap: 0.25rem;
  }
  .tab {
    background: #161616;
    border: 1px solid #222;
    border-radius: 6px 6px 0 0;
    color: #888;
    padding: 0.4rem 0.8rem;
    font-family: inherit;
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    cursor: pointer;
    transition: all 0.15s;
  }
  .tab:hover {
    background: #1a1a1a;
    color: #aaa;
  }
  .tab.active {
    background: #111;
    color: #7af;
    border-bottom-color: #111;
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin-left: auto;
  }
  .controls label {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }
  .label {
    color: #7af;
    font-size: 0.7rem;
  }
  select {
    background: #161616;
    border: 1px solid #333;
    border-radius: 4px;
    color: #ccc;
    padding: 0.2rem 0.4rem;
    font-family: inherit;
    font-size: 0.7rem;
  }
  select:focus {
    border-color: #7af;
    outline: none;
  }
  .count {
    color: #555;
    font-size: 0.7rem;
  }
  .log-list,
  .query-list {
    flex: 1;
    overflow-y: auto;
    font-size: 0.75rem;
    line-height: 1.5;
  }
  .log-item {
    margin-bottom: 0.15rem;
  }
  .log-entry {
    display: flex;
    gap: 0.5rem;
    padding: 0.25rem 0.3rem;
    border-bottom: 1px solid #1a1a1a;
    white-space: nowrap;
  }
  .log-entry.expandable {
    cursor: pointer;
    padding: 0.3rem 0.5rem;
    border-radius: 4px;
    transition: background 0.15s;
  }
  .log-entry.expandable:hover {
    background: #161616;
  }
  .log-entry.expanded {
    background: #161616;
  }
  .time {
    color: #555;
    flex-shrink: 0;
    font-size: 0.7rem;
  }
  .level {
    font-weight: 600;
    flex-shrink: 0;
    width: 3.5rem;
  }
  .target {
    color: #777;
    font-size: 0.7rem;
    flex-shrink: 0;
    min-width: 12rem;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .message {
    color: #ccc;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
  }
  .expand-icon {
    color: #666;
    font-size: 0.6rem;
    flex-shrink: 0;
    margin-left: 0.3rem;
  }
  .fields-panel {
    margin: 0.3rem 0.5rem 0.5rem 0.5rem;
    padding: 0.5rem;
    background: #0f0f0f;
    border: 1px solid #1a1a1a;
    border-radius: 4px;
    font-size: 0.7rem;
  }
  .field-row {
    display: flex;
    gap: 0.5rem;
    padding: 0.15rem 0;
    border-bottom: 1px solid #151515;
  }
  .field-row:last-child {
    border-bottom: none;
  }
  .field-key {
    color: #7af;
    flex-shrink: 0;
    min-width: 6rem;
    font-weight: 500;
  }
  .field-value {
    color: #bbb;
    overflow-wrap: break-word;
  }
  .empty {
    color: #555;
    font-size: 0.8rem;
    padding: 2rem;
    text-align: center;
  }
</style>
