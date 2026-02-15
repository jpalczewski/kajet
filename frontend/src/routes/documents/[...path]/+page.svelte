<script lang="ts">
  import { page } from '$app/state';
  import { getDocumentDetail } from '$lib/api';
  import type { DocumentDetail } from '$lib/types';
  import { t } from '$lib/stores/i18n.svelte';
  import DocumentDetailView from '$lib/components/DocumentDetail.svelte';

  let detail = $state<DocumentDetail | null>(null);
  let loading = $state(false);
  let error = $state('');
  let abortController: AbortController | null = null;

  const pathParam = $derived(page.params.path || '');

  $effect(() => {
    abortController?.abort();

    if (pathParam) {
      abortController = new AbortController();
      void loadDetail(pathParam, abortController.signal);
    } else {
      detail = null;
      error = '';
      abortController = null;
    }
  });

  async function loadDetail(path: string, signal: AbortSignal) {
    loading = true;
    error = '';
    try {
      detail = await getDocumentDetail(path, signal);
    } catch (e) {
      if (e instanceof Error && e.name === 'AbortError') {
        return;
      }
      error = String(e);
      detail = null;
    } finally {
      loading = false;
    }
  }
</script>

<div class="documents-page">
  <h1 class="page-title">{t('documents_title', 'Documents')}</h1>

  {#if loading}
    <div class="loading">{t('loading', 'Loading...')}</div>
  {:else if error}
    <div class="error">{error}</div>
  {:else if detail}
    <DocumentDetailView {detail} />
  {/if}
</div>

<style>
  .documents-page {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .page-title {
    font-size: 1.4rem;
    color: #fff;
    margin: 0;
  }

  .loading,
  .error {
    color: #555;
    font-size: 0.8rem;
    padding: 1rem;
    text-align: center;
  }

  .error {
    color: #d55;
  }
</style>
