<script lang="ts">
  import type { DocumentDetail } from '$lib/types';
  import { t } from '$lib/stores/i18n.svelte';
  import ChunkViewer from './ChunkViewer.svelte';

  let { detail }: { detail: DocumentDetail } = $props();

  function formatDate(timestamp: number): string {
    const date = new Date(timestamp * 1000);
    return date.toLocaleString();
  }

</script>

<div class="document-detail">
  <div class="detail-header">
    <a class="back-button" href="/documents">← {t('documents_back', 'Back')}</a>
    <h2 class="detail-title">{detail.document.title}</h2>
  </div>

  <div class="metadata">
    <div class="meta-row">
      <span class="meta-label">{t('documents_path', 'Path')}:</span>
      <span class="meta-value">{detail.document.source_file}</span>
    </div>

    {#if detail.document.tags.length > 0}
      <div class="meta-row">
        <span class="meta-label">{t('documents_tags', 'Tags')}:</span>
        <div class="meta-tags">
          {#each detail.document.tags as tag}
            <span class="tag">#{tag}</span>
          {/each}
        </div>
      </div>
    {/if}

    <div class="meta-row">
      <span class="meta-label">{t('documents_modified', 'Modified')}:</span>
      <span class="meta-value">{formatDate(detail.document.last_modified)}</span>
    </div>

    <div class="meta-row">
      <span class="meta-label">{t('documents_chunks', 'Chunks')}:</span>
      <span class="meta-value">{detail.chunks.length}</span>
    </div>

    {#if detail.document.outgoing_links.length > 0}
      <div class="meta-row">
        <span class="meta-label">{t('documents_outgoing_links', 'Outgoing links')}:</span>
        <div class="meta-links">
          {#each detail.document.outgoing_links as link}
            <span class="link">{link}</span>
          {/each}
        </div>
      </div>
    {/if}

    {#if detail.document.backlinks.length > 0}
      <div class="meta-row">
        <span class="meta-label">{t('documents_backlinks', 'Backlinks')}:</span>
        <div class="meta-links">
          {#each detail.document.backlinks as link}
            <span class="link">{link}</span>
          {/each}
        </div>
      </div>
    {/if}
  </div>

  <div class="chunks-section">
    <h3 class="section-title">{t('documents_content_chunks', 'Content chunks')}</h3>
    {#each detail.chunks as chunk}
      <ChunkViewer {chunk} />
    {/each}
  </div>
</div>

<style>
  .document-detail {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .detail-header {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .back-button {
    text-decoration: none;
    background: #222;
    border: 1px solid #333;
    color: #888;
    padding: 0.4rem 0.8rem;
    border-radius: 4px;
    font-size: 0.8rem;
    cursor: pointer;
    font-family: inherit;
  }

  .back-button:hover {
    background: #2a2a2a;
    color: #aaa;
  }

  .detail-title {
    font-size: 1.2rem;
    color: #fff;
    margin: 0;
  }

  .metadata {
    background: #161616;
    padding: 1rem;
    border-radius: 6px;
    border-left: 3px solid #333;
  }

  .meta-row {
    display: flex;
    gap: 1rem;
    padding: 0.5rem 0;
    border-bottom: 1px solid #222;
    font-size: 0.85rem;
  }

  .meta-row:last-child {
    border-bottom: none;
  }

  .meta-label {
    color: #888;
    min-width: 120px;
  }

  .meta-value {
    color: #bbb;
    flex: 1;
  }

  .meta-tags,
  .meta-links {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    flex: 1;
  }

  .tag {
    font-size: 0.7rem;
    color: #7af;
    background: #1a2a3a;
    padding: 0.1rem 0.4rem;
    border-radius: 3px;
  }

  .link {
    font-size: 0.75rem;
    color: #7af;
    padding: 0.1rem 0.3rem;
    background: #1a1a2a;
    border-radius: 3px;
  }

  .chunks-section {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .section-title {
    color: #666;
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin: 0.5rem 0 0.3rem 0;
  }
</style>
