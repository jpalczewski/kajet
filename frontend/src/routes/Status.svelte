<script lang="ts">
  import { getStatus } from '../lib/api';
  import { t } from '../lib/stores/i18n.svelte';
  import type { VaultStatus } from '../lib/types';

  let status = $state<VaultStatus | null>(null);
  let error = $state('');

  $effect(() => {
    getStatus()
      .then((s) => (status = s))
      .catch((e) => (error = String(e)));
  });
</script>

<div class="panel">
  <h2>{t('nav_status', 'Status')}</h2>
  {#if error}
    <div class="error">{error}</div>
  {:else if !status}
    <div class="loading">{t('loading', 'Loading...')}</div>
  {:else}
    <div class="stat-grid">
      <div class="stat">
        <span class="label">{t('status_vault_path', 'Vault path')}</span>
        <span class="value">{status.vault_path}</span>
      </div>
      <div class="stat">
        <span class="label">{t('status_note_count', 'Notes')}</span>
        <span class="value">{status.note_count}</span>
      </div>
      <div class="stat">
        <span class="label">{t('status_chunk_count', 'Chunks')}</span>
        <span class="value">{status.chunk_count}</span>
      </div>
      <div class="stat">
        <span class="label">{t('status_model', 'Model')}</span>
        <span class="value">{status.model}</span>
      </div>
      <div class="stat">
        <span class="label">{t('status_language', 'Language')}</span>
        <span class="value">{status.language}</span>
      </div>
    </div>
  {/if}
</div>

<style>
  .panel {
    background: #111;
    border: 1px solid #222;
    border-radius: 8px;
    padding: 1rem;
  }
  h2 {
    color: #666;
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-bottom: 0.75rem;
  }

  .stat-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.75rem;
  }
  @media (max-width: 600px) { .stat-grid { grid-template-columns: 1fr; } }

  .stat {
    background: #161616;
    padding: 0.6rem 0.8rem;
    border-radius: 6px;
    border-left: 3px solid #333;
  }
  .label { display: block; color: #666; font-size: 0.7rem; margin-bottom: 0.2rem; }
  .value { color: #ccc; font-size: 0.85rem; }
  .loading { color: #555; font-size: 0.8rem; padding: 1rem; text-align: center; }
  .error { color: #d55; font-size: 0.8rem; padding: 0.5rem; }
</style>
