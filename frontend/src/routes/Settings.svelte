<script lang="ts">
  import { getConfig, updateGlobalConfig, updateVaultConfig } from '../lib/api';
  import { t, reloadTranslations } from '../lib/stores/i18n.svelte';
  import type { KajetConfig } from '../lib/types';

  let config = $state<KajetConfig | null>(null);
  let error = $state('');

  // Global form state
  let gLanguage = $state('en');
  let gPort = $state(3579);
  let gDefaultLimit = $state(5);
  let gMaxConcurrent = $state(16);
  let gPipelineBuffer = $state(256);
  let gExcludeFolders = $state('');
  let gEmbeddingModel = $state('');
  let gOpenBrowser = $state(false);
  let globalStatus = $state('');

  // Vault form state
  let vExcludeFolders = $state('');
  let vEmbeddingModel = $state('');
  let vaultStatus = $state('');

  function loadForm(c: KajetConfig) {
    gLanguage = c.language;
    gPort = c.port;
    gDefaultLimit = c.default_limit;
    gMaxConcurrent = c.max_concurrent_files;
    gPipelineBuffer = c.pipeline_buffer_size;
    gExcludeFolders = c.exclude_folders.join(', ');
    gEmbeddingModel = c.embedding_model;
    gOpenBrowser = c.open_browser;
    // Vault fields start empty (override only)
    vExcludeFolders = '';
    vEmbeddingModel = '';
  }

  $effect(() => {
    getConfig()
      .then((c) => {
        config = c;
        loadForm(c);
      })
      .catch((e) => (error = String(e)));
  });

  async function saveGlobal() {
    globalStatus = '';
    try {
      const updates: Record<string, unknown> = {
        language: gLanguage,
        port: gPort,
        default_limit: gDefaultLimit,
        max_concurrent_files: gMaxConcurrent,
        pipeline_buffer_size: gPipelineBuffer,
        exclude_folders: gExcludeFolders.split(',').map((s) => s.trim()).filter(Boolean),
        embedding_model: gEmbeddingModel,
        open_browser: gOpenBrowser,
      };
      await updateGlobalConfig(updates);
      await reloadTranslations();
      globalStatus = t('settings_saved', 'Saved!');
    } catch (e) {
      globalStatus = `${t('settings_error', 'Error')}: ${e}`;
    }
  }

  async function saveVault() {
    vaultStatus = '';
    try {
      const updates: Record<string, unknown> = {};
      if (vExcludeFolders.trim()) {
        updates.exclude_folders = vExcludeFolders.split(',').map((s) => s.trim()).filter(Boolean);
      }
      if (vEmbeddingModel.trim()) {
        updates.embedding_model = vEmbeddingModel;
      }
      if (Object.keys(updates).length === 0) return;
      await updateVaultConfig(updates);
      vaultStatus = t('settings_saved', 'Saved!');
    } catch (e) {
      vaultStatus = `${t('settings_error', 'Error')}: ${e}`;
    }
  }
</script>

<div class="panel">
  <h2>{t('nav_settings', 'Settings')}</h2>
  {#if error}
    <div class="error">{error}</div>
  {:else if !config}
    <div class="loading">{t('loading', 'Loading...')}</div>
  {:else}
    <!-- Global Settings -->
    <section>
      <h3>{t('settings_global', 'Global')}</h3>
      <div class="form-grid">
        <label>
          <span class="label">{t('settings_language', 'Language')}</span>
          <select bind:value={gLanguage}>
            <option value="en">English</option>
            <option value="pl">Polski</option>
          </select>
        </label>

        <label>
          <span class="label">{t('settings_port', 'Port')}</span>
          <input type="number" bind:value={gPort} min="1024" max="65535" />
          <span class="hint">{t('settings_restart_required', 'Requires restart')}</span>
        </label>

        <label>
          <span class="label">{t('settings_default_limit', 'Default limit')}</span>
          <input type="number" bind:value={gDefaultLimit} min="1" max="100" />
        </label>

        <label>
          <span class="label">{t('settings_max_concurrent_files', 'Max concurrent files')}</span>
          <input type="number" bind:value={gMaxConcurrent} min="1" max="128" />
        </label>

        <label>
          <span class="label">{t('settings_pipeline_buffer_size', 'Pipeline buffer size')}</span>
          <input type="number" bind:value={gPipelineBuffer} min="1" max="4096" />
        </label>

        <label>
          <span class="label">{t('settings_exclude_folders', 'Exclude folders')}</span>
          <input type="text" bind:value={gExcludeFolders} placeholder=".obsidian, .trash, .kajet" />
        </label>

        <label>
          <span class="label">{t('settings_embedding_model', 'Embedding model')}</span>
          <input type="text" bind:value={gEmbeddingModel} />
          <span class="hint">{t('settings_restart_required', 'Requires restart')}</span>
        </label>

        <label class="checkbox-label">
          <input type="checkbox" bind:checked={gOpenBrowser} />
          <span class="label">{t('settings_open_browser', 'Open browser on start')}</span>
        </label>
      </div>
      <div class="actions">
        <button onclick={saveGlobal}>{t('settings_save', 'Save')}</button>
        {#if globalStatus}<span class="status-msg">{globalStatus}</span>{/if}
      </div>
    </section>

    <!-- Vault Settings -->
    <section>
      <h3>{t('settings_vault', 'Vault override')}</h3>
      <div class="form-grid">
        <label>
          <span class="label">{t('settings_exclude_folders', 'Exclude folders')}</span>
          <input type="text" bind:value={vExcludeFolders} placeholder=".obsidian, .trash, .kajet" />
        </label>

        <label>
          <span class="label">{t('settings_embedding_model', 'Embedding model')}</span>
          <input type="text" bind:value={vEmbeddingModel} />
          <span class="hint">{t('settings_restart_required', 'Requires restart')}</span>
        </label>
      </div>
      <div class="actions">
        <button onclick={saveVault}>{t('settings_save', 'Save')}</button>
        {#if vaultStatus}<span class="status-msg">{vaultStatus}</span>{/if}
      </div>
    </section>
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
  h3 {
    color: #888;
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    margin-bottom: 0.6rem;
    padding-bottom: 0.3rem;
    border-bottom: 1px solid #222;
  }
  section {
    margin-bottom: 1.2rem;
  }
  section:last-child {
    margin-bottom: 0;
  }
  .form-grid {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }
  .checkbox-label {
    flex-direction: row;
    align-items: center;
    gap: 0.5rem;
  }
  .label {
    color: #7af;
    font-size: 0.75rem;
  }
  .hint {
    color: #665;
    font-size: 0.65rem;
    font-style: italic;
  }
  input[type='text'],
  input[type='number'],
  select {
    background: #161616;
    border: 1px solid #333;
    border-radius: 4px;
    color: #ccc;
    padding: 0.35rem 0.5rem;
    font-family: inherit;
    font-size: 0.8rem;
  }
  input[type='text']:focus,
  input[type='number']:focus,
  select:focus {
    border-color: #7af;
    outline: none;
  }
  input[type='checkbox'] {
    accent-color: #7af;
  }
  .actions {
    margin-top: 0.6rem;
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }
  button {
    background: #222;
    border: 1px solid #444;
    border-radius: 4px;
    color: #ccc;
    padding: 0.35rem 1rem;
    font-family: inherit;
    font-size: 0.75rem;
    cursor: pointer;
  }
  button:hover {
    background: #333;
    border-color: #7af;
  }
  .status-msg {
    color: #6b6;
    font-size: 0.75rem;
  }
  .loading {
    color: #555;
    font-size: 0.8rem;
    padding: 1rem;
    text-align: center;
  }
  .error {
    color: #d55;
    font-size: 0.8rem;
    padding: 0.5rem;
  }
</style>
