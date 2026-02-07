<script lang="ts">
  import { getConfig } from '../lib/api';
  import { t } from '../lib/stores/i18n.svelte';
  import type { KajetConfig } from '../lib/types';

  let config = $state<KajetConfig | null>(null);
  let error = $state('');

  $effect(() => {
    getConfig()
      .then((c) => (config = c))
      .catch((e) => (error = String(e)));
  });
</script>

<div class="panel">
  <h2>{t('nav_settings', 'Settings')}</h2>
  {#if error}
    <div class="error">{error}</div>
  {:else if !config}
    <div class="loading">{t('loading', 'Loading...')}</div>
  {:else}
    <div class="config-list">
      <div class="config-item">
        <span class="key">port</span>
        <span class="val">{config.port}</span>
      </div>
      <div class="config-item">
        <span class="key">language</span>
        <span class="val">{config.language}</span>
      </div>
      <div class="config-item">
        <span class="key">default_limit</span>
        <span class="val">{config.default_limit}</span>
      </div>
      <div class="config-item">
        <span class="key">exclude_folders</span>
        <span class="val">{config.exclude_folders.join(', ')}</span>
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

  .config-list { display: flex; flex-direction: column; gap: 0.4rem; }
  .config-item {
    background: #161616;
    padding: 0.5rem 0.8rem;
    border-radius: 6px;
    border-left: 3px solid #333;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .key { color: #7af; font-size: 0.8rem; }
  .val { color: #ccc; font-size: 0.8rem; }
  .loading { color: #555; font-size: 0.8rem; padding: 1rem; text-align: center; }
  .error { color: #d55; font-size: 0.8rem; padding: 0.5rem; }
</style>
