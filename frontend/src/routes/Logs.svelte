<script lang="ts">
  import { getWsStore } from '../lib/stores/websocket.svelte';
  import { t } from '../lib/stores/i18n.svelte';
  import type { LogEntry } from '../lib/types';

  const ws = getWsStore();

  const LEVELS = ['TRACE', 'DEBUG', 'INFO', 'WARN', 'ERROR'];
  let minLevel = $state('INFO');
  let autoScroll = $state(true);
  let container: HTMLDivElement | undefined = $state();

  function levelIndex(level: string): number {
    return LEVELS.indexOf(level.toUpperCase());
  }

  const filtered = $derived(
    ws.logs.filter((e: LogEntry) => levelIndex(e.level) >= levelIndex(minLevel))
  );

  function shortTarget(target: string): string {
    const parts = target.split('::');
    return parts[parts.length - 1];
  }

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

  function formatFields(fields: Record<string, unknown> | undefined): string {
    if (!fields || Object.keys(fields).length === 0) return '';
    return Object.entries(fields)
      .map(([k, v]) => `${k}=${typeof v === 'string' ? v : JSON.stringify(v)}`)
      .join(' ');
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
    <h2>{t('logs_title', 'Logs')}</h2>
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
  </div>

  <div class="log-list" bind:this={container} onscroll={handleScroll}>
    {#if filtered.length === 0}
      <div class="empty">{t('logs_empty', 'No log entries')}</div>
    {:else}
      {#each filtered as entry}
        <div class="log-entry">
          <span class="time">{formatTime(entry.timestamp)}</span>
          <span class="level" style:color={levelColor(entry.level)}>{entry.level.padEnd(5)}</span>
          <span class="target">{shortTarget(entry.target)}</span>
          <span class="message">{entry.message}</span>
          {#if entry.fields && Object.keys(entry.fields).length > 0}
            <span class="fields">{formatFields(entry.fields)}</span>
          {/if}
        </div>
      {/each}
    {/if}
  </div>
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
  h2 {
    color: #666;
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin: 0;
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
  .log-list {
    flex: 1;
    overflow-y: auto;
    font-size: 0.75rem;
    line-height: 1.5;
  }
  .log-entry {
    display: flex;
    gap: 0.5rem;
    padding: 0.1rem 0;
    border-bottom: 1px solid #1a1a1a;
    white-space: nowrap;
  }
  .time {
    color: #555;
    flex-shrink: 0;
  }
  .level {
    font-weight: 600;
    flex-shrink: 0;
    width: 3.5rem;
  }
  .target {
    color: #668;
    flex-shrink: 0;
    max-width: 12rem;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .message {
    color: #ccc;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .fields {
    color: #586;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .empty {
    color: #555;
    font-size: 0.8rem;
    padding: 2rem;
    text-align: center;
  }
</style>
