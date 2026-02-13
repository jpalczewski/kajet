<script lang="ts">
  import { getDocuments } from '../lib/api';
  import type { DocumentSummary } from '../lib/types';
  import { t } from '../lib/stores/i18n.svelte';
  import { push } from 'svelte-spa-router';

  let searchQuery = $state('');
  let tagFilter = $state('');
  let documents = $state<DocumentSummary[]>([]);
  let total = $state(0);
  let loading = $state(false);
  let error = $state('');
  let offset = $state(0);
  let limit = $state(50);

  let debounceTimer: ReturnType<typeof setTimeout>;
  let abortController: AbortController | null = null;
  let mounted = $state(false);

  // Cleanup debounce timer on unmount
  $effect(() => {
    return () => clearTimeout(debounceTimer);
  });

  // Load documents only on initial mount
  $effect(() => {
    if (!mounted) {
      mounted = true;
      void loadDocuments();
    }
  });

  function onSearchInput(e: Event) {
    const value = (e.target as HTMLInputElement).value;
    searchQuery = value;
    offset = 0;
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => void loadDocuments(), 300);
  }

  function onTagInput(e: Event) {
    const value = (e.target as HTMLInputElement).value;
    tagFilter = value;
    offset = 0;
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => void loadDocuments(), 300);
  }

  async function loadDocuments() {
    // Cancel previous fetch if still running
    abortController?.abort();
    abortController = new AbortController();
    loading = true;
    error = '';
    try {
      const response = await getDocuments(
        {
          search: searchQuery || undefined,
          tag: tagFilter || undefined,
          limit,
          offset,
        },
        abortController.signal
      );
      documents = response.documents;
      total = response.total;
    } catch (e) {
      // Ignore abort errors
      if (e instanceof Error && e.name === 'AbortError') {
        return;
      }
      error = String(e);
      documents = [];
      total = 0;
    } finally {
      loading = false;
    }
  }

  function onDocumentClick(doc: DocumentSummary) {
    push(`/documents/${encodeURIComponent(doc.source_file)}`);
  }

  function formatDate(timestamp: number): string {
    const date = new Date(timestamp * 1000);
    return date.toLocaleDateString();
  }

  function nextPage() {
    if (offset + limit < total) {
      offset += limit;
      void loadDocuments();
    }
  }

  function prevPage() {
    if (offset > 0) {
      offset = Math.max(0, offset - limit);
      void loadDocuments();
    }
  }
</script>

<div class="document-list">
  <div class="filters">
    <input
      type="text"
      placeholder={t('documents_search', 'Search documents...')}
      value={searchQuery}
      oninput={onSearchInput}
      class="filter-input"
    />
    <input
      type="text"
      placeholder={t('documents_tag_filter', 'Filter by tag...')}
      value={tagFilter}
      oninput={onTagInput}
      class="filter-input"
    />
  </div>

  {#if loading}
    <div class="loading">{t('loading', 'Loading...')}</div>
  {:else if error}
    <div class="error">{error}</div>
  {:else if documents.length === 0}
    <div class="empty">{t('documents_no_results', 'No documents found')}</div>
  {:else}
    <div class="documents">
      {#each documents as doc}
        <div
          class="document-item"
          role="button"
          tabindex="0"
          onclick={() => onDocumentClick(doc)}
          onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && onDocumentClick(doc)}
        >
          <div class="doc-header">
            <span class="doc-title">{doc.title}</span>
            <span class="doc-chunks">{doc.chunk_count} chunks</span>
          </div>
          <div class="doc-path">{doc.source_file}</div>
          {#if doc.tags.length > 0}
            <div class="doc-tags">
              {#each doc.tags as tag}
                <span class="tag">#{tag}</span>
              {/each}
            </div>
          {/if}
          <div class="doc-meta">
            <span class="doc-date">{t('documents_modified', 'Modified')}: {formatDate(doc.last_modified)}</span>
          </div>
        </div>
      {/each}
    </div>

    <div class="pagination">
      <button onclick={prevPage} disabled={offset === 0}>← {t('documents_prev', 'Previous')}</button>
      <span class="pagination-info">
        {offset + 1}–{Math.min(offset + limit, total)} {t('documents_of', 'of')} {total}
      </span>
      <button onclick={nextPage} disabled={offset + limit >= total}>{t('documents_next', 'Next')} →</button>
    </div>
  {/if}
</div>

<style>
  .document-list {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .filters {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.5rem;
  }

  @media (max-width: 700px) {
    .filters {
      grid-template-columns: 1fr;
    }
  }

  .filter-input {
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

  .filter-input:focus {
    border-color: #555;
  }

  .filter-input::placeholder {
    color: #444;
  }

  .loading,
  .empty,
  .error {
    color: #555;
    font-size: 0.8rem;
    padding: 1rem;
    text-align: center;
  }

  .error {
    color: #d55;
  }

  .documents {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .document-item {
    background: #161616;
    padding: 0.8rem;
    border-radius: 6px;
    border-left: 3px solid #333;
    cursor: pointer;
    transition: all 0.15s;
  }

  .document-item:hover {
    background: #1a1a1a;
    border-left-color: #7af;
  }

  .doc-header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    margin-bottom: 0.3rem;
  }

  .doc-title {
    font-size: 0.9rem;
    color: #fff;
    font-weight: 600;
  }

  .doc-chunks {
    font-size: 0.7rem;
    color: #666;
  }

  .doc-path {
    font-size: 0.75rem;
    color: #777;
    margin-bottom: 0.3rem;
  }

  .doc-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    margin-bottom: 0.3rem;
  }

  .tag {
    font-size: 0.7rem;
    color: #7af;
    background: #1a2a3a;
    padding: 0.1rem 0.4rem;
    border-radius: 3px;
  }

  .doc-meta {
    font-size: 0.7rem;
    color: #666;
    padding-top: 0.3rem;
    border-top: 1px solid #222;
  }

  .pagination {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 0;
  }

  .pagination button {
    background: #222;
    border: 1px solid #333;
    color: #888;
    padding: 0.4rem 0.8rem;
    border-radius: 4px;
    font-size: 0.8rem;
    cursor: pointer;
    font-family: inherit;
  }

  .pagination button:hover:not(:disabled) {
    background: #2a2a2a;
    color: #aaa;
  }

  .pagination button:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }

  .pagination-info {
    font-size: 0.8rem;
    color: #888;
  }
</style>
