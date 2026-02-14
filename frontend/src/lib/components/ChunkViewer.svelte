<script lang="ts">
  import type { ChunkDetail } from '$lib/types';
  import { t } from '$lib/stores/i18n.svelte';

  let { chunk }: { chunk: ChunkDetail } = $props();
  let showRaw = $state(false);
</script>

<div class="chunk">
  <div class="chunk-header">
    <span class="breadcrumb">{chunk.breadcrumb}</span>
    <span class="chunk-index">#{chunk.chunk_index}</span>
  </div>

  <div class="content">
    {#if showRaw}
      <pre class="raw-content">{chunk.raw_content}</pre>
    {:else}
      {chunk.content}
    {/if}
  </div>

  <div class="chunk-footer">
    <button class="toggle-raw" onclick={() => showRaw = !showRaw}>
      {showRaw ? t('chunk_processed', 'processed') : t('chunk_raw', 'raw')}
    </button>
    <span class="char-count">{chunk.char_count} chars</span>
    {#if chunk.links.length > 0}
      <span class="links-count">{chunk.links.length} links</span>
    {/if}
  </div>

  {#if chunk.links.length > 0}
    <div class="links">
      <div class="links-title">{t('chunk_links', 'Links:')}</div>
      {#each chunk.links as link}
        <div class="link-item">
          {#if link.resolved_path}
            <a class="link-target clickable" href="/documents/{encodeURIComponent(link.resolved_path)}">
              {link.target}
            </a>
          {:else}
            <span class="link-target unresolved">{link.target}</span>
          {/if}
          {#if link.alias}
            <span class="link-alias">→ {link.alias}</span>
          {/if}
          {#if link.resolved_path}
            <span class="link-resolved">({link.resolved_path})</span>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .chunk {
    background: #161616;
    padding: 0.8rem;
    margin: 0.5rem 0;
    border-radius: 6px;
    border-left: 3px solid #333;
  }

  .chunk-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
  }

  .breadcrumb {
    color: #777;
    font-size: 0.7rem;
  }

  .chunk-index {
    color: #555;
    font-size: 0.7rem;
    font-weight: 600;
  }

  .content {
    font-size: 0.8rem;
    line-height: 1.6;
    color: #bbb;
    white-space: pre-wrap;
    margin-bottom: 0.5rem;
  }

  .raw-content {
    color: #999;
    font-size: 0.75rem;
    margin: 0;
  }

  .chunk-footer {
    display: flex;
    gap: 0.75rem;
    align-items: center;
    padding-top: 0.5rem;
    border-top: 1px solid #222;
  }

  .toggle-raw {
    background: #222;
    border: 1px solid #333;
    color: #888;
    padding: 0.2rem 0.5rem;
    border-radius: 4px;
    font-size: 0.7rem;
    cursor: pointer;
    font-family: inherit;
  }

  .toggle-raw:hover {
    background: #2a2a2a;
    color: #aaa;
  }

  .char-count,
  .links-count {
    color: #666;
    font-size: 0.7rem;
  }

  .links {
    margin-top: 0.5rem;
    padding-top: 0.5rem;
    border-top: 1px solid #222;
  }

  .links-title {
    color: #666;
    font-size: 0.7rem;
    margin-bottom: 0.3rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .link-item {
    font-size: 0.75rem;
    padding: 0.2rem 0;
    color: #888;
  }

  .link-target {
    color: #7af;
  }

  .link-target.clickable {
    text-decoration: none;
    transition: color 0.15s;
  }

  .link-target.clickable:hover {
    color: #9cf;
    text-decoration: underline;
  }

  .link-target.unresolved {
    color: #d55;
    text-decoration: line-through;
  }

  .link-alias {
    color: #999;
    margin-left: 0.5rem;
  }

  .link-resolved {
    color: #666;
    margin-left: 0.5rem;
  }
</style>
