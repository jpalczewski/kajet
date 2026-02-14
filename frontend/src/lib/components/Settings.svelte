<script lang="ts">
  import { getConfig, getConfigSchema, updateGlobalConfig, updateVaultConfig, getVaultConfig, getGlobalConfig, deleteVaultConfigFields } from '$lib/api';
  import { t, reloadTranslations } from '$lib/stores/i18n.svelte';
  import type { KajetConfig } from '$lib/types';
  import type { ConfigSchema } from '$lib/types/generated/ConfigSchema';
  import DynamicSettings from './DynamicSettings.svelte';

  let config = $state<KajetConfig | null>(null);
  let schema = $state<ConfigSchema | null>(null);
  let error = $state('');
  let globalStatus = $state('');
  let vaultStatus = $state('');
  let rawVaultConfig = $state<Record<string, unknown>>({});
  let rawGlobalConfig = $state<Record<string, unknown>>({});
  let activeTab = $state<'global' | 'vault'>('global');

  // Separate state for global and vault configs
  let globalConfig = $state<Record<string, unknown>>({});
  let vaultConfig = $state<Record<string, unknown>>({});

  // Track fields that need to be deleted from vault config
  let fieldsToDelete = $state<Set<string>>(new Set());

  let abortController: AbortController | null = null;

  $effect(() => {
    abortController = new AbortController();
    const signal = abortController.signal;

    Promise.all([getConfig(), getConfigSchema(signal), getVaultConfig(signal), getGlobalConfig(signal)])
      .then(([c, s, v, g]) => {
        if (signal.aborted) return;
        config = c;
        schema = s;
        rawVaultConfig = v;
        rawGlobalConfig = g;
        loadGlobalConfig(c);
        loadVaultConfig(v);
      })
      .catch((e) => {
        if (signal.aborted) return;
        error = String(e);
      });

    return () => {
      abortController?.abort();
    };
  });

  function loadGlobalConfig(c: KajetConfig) {
    globalConfig = {
      language: c.language,
      port: c.port,
      default_limit: c.default_limit,
      max_concurrent_files: c.max_concurrent_files,
      pipeline_buffer_size: c.pipeline_buffer_size,
      exclude_folders: c.exclude_folders,
      open_browser: c.open_browser,
      resolve_wikilinks: c.resolve_wikilinks,
      filter_overfetch_multiplier: c.filter_overfetch_multiplier,
      tags_only_fetch_limit: c.tags_only_fetch_limit,
      embedding: {
        backend: c.embedding.backend,
        model: c.embedding.model,
        base_url: c.embedding.base_url,
        api_key: '', // Intentionally redacted
        document_prefix: c.embedding.document_prefix,
        query_prefix: c.embedding.query_prefix,
        remote_max_batch_size: c.embedding.remote_max_batch_size,
        remote_max_input_chars: c.embedding.remote_max_input_chars,
      },
      logging: {
        level: c.logging.level,
        file_level: c.logging.file_level,
        dashboard_level: c.logging.dashboard_level,
        progress_percent_step: c.logging.progress_percent_step,
      },
      tree: {
        depth: c.tree.depth,
        size: c.tree.size,
        max_chars: c.tree.max_chars,
      },
      writer: {
        backup_enabled: c.writer.backup_enabled,
        backup_max_per_file: c.writer.backup_max_per_file,
        timestamps: {
          enabled: c.writer.timestamps.enabled,
          created_field: c.writer.timestamps.created_field,
          modified_field: c.writer.timestamps.modified_field,
          format: c.writer.timestamps.format,
          timezone: c.writer.timestamps.timezone,
        },
        frontmatter: {
          default_tags: c.writer.frontmatter.default_tags,
          created_date_field: c.writer.frontmatter.created_date_field,
          modified_date_field: c.writer.frontmatter.modified_date_field,
        },
      },
    };
  }

  function loadVaultConfig(raw: Record<string, unknown>) {
    vaultConfig = raw;
    // Clear deletion list when loading fresh config
    fieldsToDelete = new Set();
  }

  function handleFieldReset(sectionKey: string, fieldKey: string) {
    // Build field path for deletion (e.g., "exclude_folders" or "embedding.model")
    const fieldPath = sectionKey === 'general' || sectionKey === 'search_tuning'
      ? fieldKey
      : `${sectionKey}.${fieldKey}`;

    console.log('[handleFieldReset] Marking for deletion:', fieldPath);
    fieldsToDelete.add(fieldPath);
  }

  // Helper to recursively remove null/undefined values from objects
  function cleanNulls(obj: unknown): unknown {
    if (obj === null || obj === undefined) return undefined;
    if (typeof obj !== 'object') return obj;
    if (Array.isArray(obj)) return obj;

    const cleaned: Record<string, unknown> = {};
    for (const [key, value] of Object.entries(obj as Record<string, unknown>)) {
      if (value === null || value === undefined) continue;
      if (typeof value === 'object' && !Array.isArray(value)) {
        const cleanedNested = cleanNulls(value);
        if (cleanedNested && Object.keys(cleanedNested as object).length > 0) {
          cleaned[key] = cleanedNested;
        }
      } else if (value !== '') {
        cleaned[key] = value;
      }
    }
    return cleaned;
  }

  async function saveGlobal() {
    globalStatus = '';
    try {
      const updates: Record<string, unknown> = {
        language: globalConfig.language,
        port: globalConfig.port,
        default_limit: globalConfig.default_limit,
        max_concurrent_files: globalConfig.max_concurrent_files,
        pipeline_buffer_size: globalConfig.pipeline_buffer_size,
        exclude_folders: globalConfig.exclude_folders,
        open_browser: globalConfig.open_browser,
        resolve_wikilinks: globalConfig.resolve_wikilinks,
        filter_overfetch_multiplier: globalConfig.filter_overfetch_multiplier,
        tags_only_fetch_limit: globalConfig.tags_only_fetch_limit,
      };

      // Handle embedding config - clean nulls and filter api_key
      const embeddingRaw = globalConfig.embedding as Record<string, unknown>;
      const embeddingCleaned = cleanNulls(embeddingRaw) as Record<string, unknown>;
      const apiKey = embeddingCleaned.api_key as string;
      if (!apiKey || apiKey.trim() === '') {
        delete embeddingCleaned.api_key; // Don't send empty api_key
      }
      updates.embedding = embeddingCleaned;

      // Handle other nested configs - clean nulls
      updates.logging = cleanNulls(globalConfig.logging);
      updates.tree = cleanNulls(globalConfig.tree);
      updates.writer = cleanNulls(globalConfig.writer);

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
      console.log('[saveVault] vaultConfig:', vaultConfig);
      console.log('[saveVault] fieldsToDelete:', Array.from(fieldsToDelete));

      // Step 1: Delete fields that were reset
      if (fieldsToDelete.size > 0) {
        await deleteVaultConfigFields(Array.from(fieldsToDelete));
        console.log('[saveVault] Deleted fields:', Array.from(fieldsToDelete));
      }

      const updates: Record<string, unknown> = {};

      // Only send non-empty fields
      const excludeFolders = vaultConfig.exclude_folders as unknown[];
      if (excludeFolders && excludeFolders.length > 0) {
        updates.exclude_folders = excludeFolders;
      }

      const embedding = vaultConfig.embedding as Record<string, unknown>;
      if (embedding && Object.keys(embedding).length > 0) {
        const embeddingUpdates: Record<string, unknown> = {};
        for (const [key, value] of Object.entries(embedding)) {
          if (value !== '' && value !== null && value !== undefined) {
            embeddingUpdates[key] = value;
          }
        }
        if (Object.keys(embeddingUpdates).length > 0) {
          updates.embedding = embeddingUpdates;
        }
      }

      const tree = vaultConfig.tree as Record<string, unknown>;
      if (tree && Object.keys(tree).length > 0) {
        const treeUpdates: Record<string, unknown> = {};
        for (const [key, value] of Object.entries(tree)) {
          if (value !== '' && value !== null && value !== undefined) {
            treeUpdates[key] = value;
          }
        }
        if (Object.keys(treeUpdates).length > 0) {
          updates.tree = treeUpdates;
        }
      }

      const writer = vaultConfig.writer as Record<string, unknown>;
      if (writer && Object.keys(writer).length > 0) {
        const writerUpdates: Record<string, unknown> = {};

        // Handle nested structures (timestamps, frontmatter)
        const timestamps = writer.timestamps as Record<string, unknown>;
        if (timestamps && Object.keys(timestamps).length > 0) {
          const tsUpdates: Record<string, unknown> = {};
          for (const [key, value] of Object.entries(timestamps)) {
            if (value !== '' && value !== null && value !== undefined) {
              tsUpdates[key] = value;
            }
          }
          if (Object.keys(tsUpdates).length > 0) {
            writerUpdates.timestamps = tsUpdates;
          }
        }

        const frontmatter = writer.frontmatter as Record<string, unknown>;
        if (frontmatter && Object.keys(frontmatter).length > 0) {
          const fmUpdates: Record<string, unknown> = {};
          for (const [key, value] of Object.entries(frontmatter)) {
            if (value !== '' && value !== null && value !== undefined) {
              fmUpdates[key] = value;
            }
          }
          if (Object.keys(fmUpdates).length > 0) {
            writerUpdates.frontmatter = fmUpdates;
          }
        }

        // Add other writer fields
        for (const [key, value] of Object.entries(writer)) {
          if (key !== 'timestamps' && key !== 'frontmatter' && value !== '' && value !== null && value !== undefined) {
            writerUpdates[key] = value;
          }
        }

        if (Object.keys(writerUpdates).length > 0) {
          updates.writer = writerUpdates;
        }
      }

      console.log('[saveVault] updates:', updates);

      // Step 2: Update fields that were changed (if any)
      if (Object.keys(updates).length > 0) {
        await updateVaultConfig(updates);
        console.log('[saveVault] updateVaultConfig succeeded');
      }

      // Step 3: Reload configs to update badges in both tabs
      const [v, c] = await Promise.all([getVaultConfig(), getConfig()]);
      rawVaultConfig = v;
      config = c;
      loadVaultConfig(v); // This also clears fieldsToDelete
      loadGlobalConfig(c); // Update global tab with new merged values

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
  {:else if !config || !schema}
    <div class="loading">{t('loading', 'Loading...')}</div>
  {:else}
    <div class="tabs">
      <button
        class:active={activeTab === 'global'}
        onclick={() => (activeTab = 'global')}
      >
        {t('settings_global', 'Global')}
      </button>
      <button
        class:active={activeTab === 'vault'}
        onclick={() => (activeTab = 'vault')}
      >
        {t('settings_vault', 'Vault override')}
      </button>
    </div>

    <div class="tab-content">
      {#if activeTab === 'global'}
        <DynamicSettings
          {schema}
          config={globalConfig}
          vaultConfig={rawVaultConfig}
          scope="Global"
          onsave={saveGlobal}
          status={globalStatus}
        />
      {:else}
        <DynamicSettings
          {schema}
          config={vaultConfig}
          globalConfig={config}
          scope="Vault"
          onsave={saveVault}
          onreset={handleFieldReset}
          status={vaultStatus}
        />
      {/if}
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
  .tabs {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 1rem;
    border-bottom: 1px solid #222;
    padding-bottom: 0.5rem;
  }
  .tabs button {
    background: transparent;
    border: none;
    color: #888;
    padding: 0.4rem 0.8rem;
    font-family: inherit;
    font-size: 0.75rem;
    cursor: pointer;
    border-bottom: 2px solid transparent;
  }
  .tabs button:hover {
    color: #aaa;
  }
  .tabs button.active {
    color: #7af;
    border-bottom-color: #7af;
  }
  .tab-content {
    margin-top: 1rem;
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
