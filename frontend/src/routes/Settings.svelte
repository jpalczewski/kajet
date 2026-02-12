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
  let gEmbeddingBackend = $state<'candle' | 'remote'>('candle');
  let gEmbeddingModel = $state('');
  let gEmbeddingBaseUrl = $state('');
  let gEmbeddingApiKey = $state('');
  let gDocumentPrefix = $state('');
  let gQueryPrefix = $state('');
  let gOpenBrowser = $state(false);
  let gFilterOverfetch = $state(3);
  let gTagsOnlyLimit = $state(500);
  let gLogLevel = $state('debug');
  let gFileLevel = $state('trace');
  let gDashboardLevel = $state('info');
  let gProgressStep = $state(5);
  let gTreeDepth = $state(3);
  let gTreeSize = $state(50);
  let gTreeMaxChars = $state(5000);
  let globalStatus = $state('');

  const LOG_LEVELS = ['trace', 'debug', 'info', 'warn', 'error'];

  // Vault form state
  let vExcludeFolders = $state('');
  let vEmbeddingBackend = $state<'candle' | 'remote' | ''>('');
  let vEmbeddingModel = $state('');
  let vEmbeddingBaseUrl = $state('');
  let vEmbeddingApiKey = $state('');
  let vDocumentPrefix = $state('');
  let vQueryPrefix = $state('');
  let vCreatedDateField = $state('');
  let vModifiedDateField = $state('');
  let vTreeDepth = $state<number | ''>('');
  let vTreeSize = $state<number | ''>('');
  let vTreeMaxChars = $state<number | ''>('');
  let vaultStatus = $state('');

  function loadForm(c: KajetConfig) {
    gLanguage = c.language;
    gPort = c.port;
    gDefaultLimit = c.default_limit;
    gMaxConcurrent = c.max_concurrent_files;
    gPipelineBuffer = c.pipeline_buffer_size;
    gExcludeFolders = c.exclude_folders.join(', ');
    gEmbeddingBackend = c.embedding.backend;
    gEmbeddingModel = c.embedding.model;
    gEmbeddingBaseUrl = c.embedding.base_url;
    gEmbeddingApiKey = c.embedding.api_key;
    gDocumentPrefix = c.embedding.document_prefix;
    gQueryPrefix = c.embedding.query_prefix;
    gOpenBrowser = c.open_browser;
    gFilterOverfetch = c.filter_overfetch_multiplier;
    gTagsOnlyLimit = c.tags_only_fetch_limit;
    gLogLevel = c.logging.level;
    gFileLevel = c.logging.file_level;
    gDashboardLevel = c.logging.dashboard_level;
    gProgressStep = c.logging.progress_percent_step;
    gTreeDepth = c.tree.depth;
    gTreeSize = c.tree.size;
    gTreeMaxChars = c.tree.max_chars;
    // Vault fields start empty (override only)
    vExcludeFolders = '';
    vEmbeddingBackend = '';
    vEmbeddingModel = '';
    vEmbeddingBaseUrl = '';
    vEmbeddingApiKey = '';
    vDocumentPrefix = '';
    vQueryPrefix = '';
    vCreatedDateField = '';
    vModifiedDateField = '';
    vTreeDepth = '';
    vTreeSize = '';
    vTreeMaxChars = '';
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
        embedding: {
          backend: gEmbeddingBackend,
          model: gEmbeddingModel,
          base_url: gEmbeddingBaseUrl,
          api_key: gEmbeddingApiKey,
          document_prefix: gDocumentPrefix,
          query_prefix: gQueryPrefix,
        },
        open_browser: gOpenBrowser,
        filter_overfetch_multiplier: gFilterOverfetch,
        tags_only_fetch_limit: gTagsOnlyLimit,
        logging: {
          level: gLogLevel,
          file_level: gFileLevel,
          dashboard_level: gDashboardLevel,
          progress_percent_step: gProgressStep,
        },
        tree: {
          depth: gTreeDepth,
          size: gTreeSize,
          max_chars: gTreeMaxChars,
        },
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
      const embeddingUpdates: Record<string, unknown> = {};
      if (vEmbeddingBackend) embeddingUpdates.backend = vEmbeddingBackend;
      if (vEmbeddingModel.trim()) embeddingUpdates.model = vEmbeddingModel;
      if (vEmbeddingBaseUrl.trim()) embeddingUpdates.base_url = vEmbeddingBaseUrl;
      if (vEmbeddingApiKey.trim()) embeddingUpdates.api_key = vEmbeddingApiKey;
      if (vDocumentPrefix.trim()) embeddingUpdates.document_prefix = vDocumentPrefix;
      if (vQueryPrefix.trim()) embeddingUpdates.query_prefix = vQueryPrefix;
      if (Object.keys(embeddingUpdates).length > 0) {
        updates.embedding = embeddingUpdates;
      }
      // Date fields can be empty (to use filesystem metadata) or non-empty
      const writerUpdates: Record<string, unknown> = {};
      if (vCreatedDateField !== undefined && vCreatedDateField !== null) {
        if (!writerUpdates.frontmatter) {
          writerUpdates.frontmatter = {};
        }
        (writerUpdates.frontmatter as Record<string, unknown>).created_date_field = vCreatedDateField.trim() || null;
      }
      if (vModifiedDateField !== undefined && vModifiedDateField !== null) {
        if (!writerUpdates.frontmatter) {
          writerUpdates.frontmatter = {};
        }
        (writerUpdates.frontmatter as Record<string, unknown>).modified_date_field = vModifiedDateField.trim() || null;
      }
      if (Object.keys(writerUpdates).length > 0) {
        updates.writer = writerUpdates;
      }
      const treeUpdates: Record<string, unknown> = {};
      if (vTreeDepth !== '') treeUpdates.depth = vTreeDepth;
      if (vTreeSize !== '') treeUpdates.size = vTreeSize;
      if (vTreeMaxChars !== '') treeUpdates.max_chars = vTreeMaxChars;
      if (Object.keys(treeUpdates).length > 0) {
        updates.tree = treeUpdates;
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
          <span class="label">{t('settings_embedding_backend', 'Embedding backend')}</span>
          <select bind:value={gEmbeddingBackend}>
            <option value="candle">candle</option>
            <option value="remote">remote</option>
          </select>
          <span class="hint">{t('settings_restart_required', 'Requires restart')}</span>
        </label>

        <label>
          <span class="label">{t('settings_embedding_model', 'Embedding model')}</span>
          <input type="text" bind:value={gEmbeddingModel} />
          <span class="hint">{t('settings_restart_required', 'Requires restart')}</span>
        </label>

        <label>
          <span class="label">{t('settings_embedding_base_url', 'Embedding base URL')}</span>
          <input type="text" bind:value={gEmbeddingBaseUrl} placeholder="http://localhost:1234" />
          <span class="hint">{t('settings_restart_required', 'Requires restart')}</span>
        </label>

        <label>
          <span class="label">{t('settings_embedding_api_key', 'Embedding API key')}</span>
          <input type="password" bind:value={gEmbeddingApiKey} />
          <span class="hint">{t('settings_restart_required', 'Requires restart')}</span>
        </label>

        <label>
          <span class="label">{t('settings_embedding_document_prefix', 'Document prefix')}</span>
          <input type="text" bind:value={gDocumentPrefix} placeholder="search_document: " />
          <span class="hint">{t('settings_restart_required', 'Requires restart')}</span>
        </label>

        <label>
          <span class="label">{t('settings_embedding_query_prefix', 'Query prefix')}</span>
          <input type="text" bind:value={gQueryPrefix} placeholder="search_query: " />
          <span class="hint">{t('settings_restart_required', 'Requires restart')}</span>
        </label>

        <label class="checkbox-label">
          <input type="checkbox" bind:checked={gOpenBrowser} />
          <span class="label">{t('settings_open_browser', 'Open browser on start')}</span>
        </label>
      </div>

      <!-- Search tuning subsection -->
      <h3>{t('settings_search_tuning', 'Search tuning')}</h3>
      <div class="form-grid">
        <label>
          <span class="label">{t('settings_filter_overfetch_multiplier', 'Filter overfetch multiplier')}</span>
          <input type="number" bind:value={gFilterOverfetch} min="1" max="10" />
          <span class="hint">{t('settings_filter_overfetch_multiplier_hint', 'How many extra results to fetch when filters are active')}</span>
        </label>

        <label>
          <span class="label">{t('settings_tags_only_fetch_limit', 'Tags-only fetch limit')}</span>
          <input type="number" bind:value={gTagsOnlyLimit} min="10" max="2000" />
          <span class="hint">{t('settings_tags_only_fetch_limit_hint', 'Maximum documents to scan when filtering by tags only')}</span>
        </label>
      </div>

      <!-- Tree tool subsection -->
      <h3>{t('settings_tree', 'Tree tool')}</h3>
      <div class="form-grid">
        <label>
          <span class="label">{t('settings_tree_depth', 'Default depth')}</span>
          <input type="number" bind:value={gTreeDepth} min="1" max="20" />
          <span class="hint">{t('settings_tree_depth_hint', 'Maximum folder depth')}</span>
        </label>
        <label>
          <span class="label">{t('settings_tree_size', 'Default size')}</span>
          <input type="number" bind:value={gTreeSize} min="10" max="500" />
          <span class="hint">{t('settings_tree_size_hint', 'Maximum entries in output')}</span>
        </label>
        <label>
          <span class="label">{t('settings_tree_max_chars', 'Max output chars')}</span>
          <input type="number" bind:value={gTreeMaxChars} min="1000" max="50000" />
          <span class="hint">{t('settings_tree_max_chars_hint', 'Refuse if output exceeds this')}</span>
        </label>
      </div>

      <!-- Logging subsection -->
      <h3>{t('settings_logging', 'Logging')}</h3>
      <div class="form-grid">
        <label>
          <span class="label">{t('settings_log_level', 'Global level')}</span>
          <select bind:value={gLogLevel}>
            {#each LOG_LEVELS as lvl}
              <option value={lvl}>{lvl.toUpperCase()}</option>
            {/each}
          </select>
          <span class="hint">{t('settings_restart_required', 'Requires restart')}</span>
        </label>

        <label>
          <span class="label">{t('settings_file_level', 'File level')}</span>
          <select bind:value={gFileLevel}>
            {#each LOG_LEVELS as lvl}
              <option value={lvl}>{lvl.toUpperCase()}</option>
            {/each}
          </select>
          <span class="hint">{t('settings_restart_required', 'Requires restart')}</span>
        </label>

        <label>
          <span class="label">{t('settings_dashboard_level', 'Dashboard level')}</span>
          <select bind:value={gDashboardLevel}>
            {#each LOG_LEVELS as lvl}
              <option value={lvl}>{lvl.toUpperCase()}</option>
            {/each}
          </select>
          <span class="hint">{t('settings_restart_required', 'Requires restart')}</span>
        </label>

        <label>
          <span class="label">{t('settings_progress_step', 'Progress step (%)')}</span>
          <input type="number" bind:value={gProgressStep} min="1" max="50" />
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
          <span class="label">{t('settings_embedding_backend', 'Embedding backend')}</span>
          <select bind:value={vEmbeddingBackend}>
            <option value="">(no override)</option>
            <option value="candle">candle</option>
            <option value="remote">remote</option>
          </select>
          <span class="hint">{t('settings_restart_required', 'Requires restart')}</span>
        </label>

        <label>
          <span class="label">{t('settings_embedding_model', 'Embedding model')}</span>
          <input type="text" bind:value={vEmbeddingModel} />
          <span class="hint">{t('settings_restart_required', 'Requires restart')}</span>
        </label>

        <label>
          <span class="label">{t('settings_embedding_base_url', 'Embedding base URL')}</span>
          <input type="text" bind:value={vEmbeddingBaseUrl} placeholder="http://localhost:1234" />
          <span class="hint">{t('settings_restart_required', 'Requires restart')}</span>
        </label>

        <label>
          <span class="label">{t('settings_embedding_api_key', 'Embedding API key')}</span>
          <input type="password" bind:value={vEmbeddingApiKey} />
          <span class="hint">{t('settings_restart_required', 'Requires restart')}</span>
        </label>

        <label>
          <span class="label">{t('settings_embedding_document_prefix', 'Document prefix')}</span>
          <input type="text" bind:value={vDocumentPrefix} placeholder="search_document: " />
          <span class="hint">{t('settings_restart_required', 'Requires restart')}</span>
        </label>

        <label>
          <span class="label">{t('settings_embedding_query_prefix', 'Query prefix')}</span>
          <input type="text" bind:value={vQueryPrefix} placeholder="search_query: " />
          <span class="hint">{t('settings_restart_required', 'Requires restart')}</span>
        </label>

        <label>
          <span class="label">{t('settings_created_date_field', 'Created date field')}</span>
          <input type="text" bind:value={vCreatedDateField} placeholder="created, Data utworzenia" />
          <span class="hint">{t('settings_created_date_field_hint', 'Field name for creation date in frontmatter')}</span>
        </label>

        <label>
          <span class="label">{t('settings_modified_date_field', 'Modified date field')}</span>
          <input type="text" bind:value={vModifiedDateField} placeholder="modified, updated" />
          <span class="hint">{t('settings_modified_date_field_hint', 'Field name for modification date in frontmatter')}</span>
        </label>

        <label>
          <span class="label">{t('settings_tree_depth', 'Default depth')}</span>
          <input type="number" bind:value={vTreeDepth} min="1" max="20" placeholder="3" />
        </label>
        <label>
          <span class="label">{t('settings_tree_size', 'Default size')}</span>
          <input type="number" bind:value={vTreeSize} min="10" max="500" placeholder="50" />
        </label>
        <label>
          <span class="label">{t('settings_tree_max_chars', 'Max output chars')}</span>
          <input type="number" bind:value={vTreeMaxChars} min="1000" max="50000" placeholder="5000" />
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
