<script lang="ts">
  import { page } from '$app/state';
  import { t } from '$lib/stores/i18n.svelte';
  import DocumentList from '$lib/components/DocumentList.svelte';

  const tagFromUrl = $derived(page.url.searchParams.get('tag') || '');
</script>

<div class="documents-page">
  <h1 class="page-title">{t('documents_title', 'Documents')}</h1>
  <DocumentList initialTag={tagFromUrl} />
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
</style>
