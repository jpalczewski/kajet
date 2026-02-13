<script lang="ts">
  import { getDocumentDetail } from '../lib/api';
  import type { DocumentDetail } from '../lib/types';
  import { t } from '../lib/stores/i18n.svelte';
  import DocumentList from '../components/DocumentList.svelte';
  import DocumentDetailView from '../components/DocumentDetail.svelte';

  let { params = {} }: { params?: Record<string, string> } = $props();

  let detail = $state<DocumentDetail | null>(null);
  let loading = $state(false);
  let error = $state('');

  // Extract path from wildcard param (svelte-spa-router gives us params['*'])
  const pathParam = $derived(params['*'] || '');

  $effect(() => {
    if (pathParam) {
      void loadDetail(pathParam);
    } else {
      detail = null;
      error = '';
    }
  });

  async function loadDetail(path: string) {
    loading = true;
    error = '';
    try {
      detail = await getDocumentDetail(path);
    } catch (e) {
      error = String(e);
      detail = null;
    } finally {
      loading = false;
    }
  }
</script>

<div class="documents-page">
  <h1 class="page-title">{t('documents_title', 'Documents')}</h1>

  {#if pathParam}
    {#if loading}
      <div class="loading">{t('loading', 'Loading...')}</div>
    {:else if error}
      <div class="error">{error}</div>
    {:else if detail}
      <DocumentDetailView {detail} />
    {/if}
  {:else}
    <DocumentList />
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
