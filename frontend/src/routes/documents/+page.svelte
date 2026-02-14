<script lang="ts">
  import { t } from '$lib/stores/i18n.svelte';
  import DocumentList from '$lib/components/DocumentList.svelte';
</script>

<div class="documents-page">
  <h1 class="page-title">{t('documents_title', 'Documents')}</h1>
  <DocumentList />
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
