<script lang="ts">
  import type { ConfigSchema } from '$lib/types/generated/ConfigSchema';
  import type { FieldScope } from '$lib/types/generated/FieldScope';
  import DynamicField from './DynamicField.svelte';
  import { t } from '$lib/stores/i18n.svelte';

  interface Props {
    schema: ConfigSchema;
    config: Record<string, unknown>;
    globalConfig?: Record<string, unknown>;
    scope: FieldScope;
    onsave: () => void;
    status?: string;
  }

  let { schema, config, globalConfig, scope, onsave, status = '' }: Props = $props();

  // Helper to get field value from config (or globalConfig if provided)
  function getFieldValue(
    sectionKey: string,
    fieldKey: string,
    cfg?: Record<string, unknown>
  ): unknown {
    const targetConfig = cfg ?? config;

    // Runtime validation: check config exists
    if (!targetConfig || typeof targetConfig !== 'object') {
      console.warn(`Config is not an object: ${typeof targetConfig}`);
      return undefined;
    }

    // Special case: search_tuning fields are stored at root level
    if (sectionKey === 'search_tuning') {
      return fieldKey in targetConfig ? targetConfig[fieldKey] : undefined;
    }

    // General section fields can be at root or nested
    if (sectionKey === 'general') {
      return fieldKey in targetConfig ? targetConfig[fieldKey] : undefined;
    }

    // For other sections, check nested object with runtime validation
    const section = targetConfig[sectionKey];
    if (!section || typeof section !== 'object') {
      return undefined;
    }

    const sectionObj = section as Record<string, unknown>;

    // Handle flattened nested properties (e.g., timestamps_enabled -> timestamps.enabled)
    if (fieldKey.includes('_')) {
      const parts = fieldKey.split('_');
      if (parts.length === 2) {
        const [prefix, suffix] = parts;
        const nested = sectionObj[prefix];
        if (nested && typeof nested === 'object') {
          const nestedObj = nested as Record<string, unknown>;
          return suffix in nestedObj ? nestedObj[suffix] : undefined;
        }
      } else if (parts.length === 3) {
        // e.g., frontmatter_created_date_field -> frontmatter.created_date_field
        const prefix = parts[0];
        const suffix = parts.slice(1).join('_');
        const nested = sectionObj[prefix];
        if (nested && typeof nested === 'object') {
          const nestedObj = nested as Record<string, unknown>;
          return suffix in nestedObj ? nestedObj[suffix] : undefined;
        }
      }
    }

    return fieldKey in sectionObj ? sectionObj[fieldKey] : undefined;
  }

  // Helper to update field value in config
  function updateFieldValue(sectionKey: string, fieldKey: string, newValue: unknown) {
    // Runtime validation: ensure config is an object
    if (!config || typeof config !== 'object') {
      console.error(`Cannot update field: config is not an object`);
      return;
    }

    // Special case: search_tuning fields are stored at root level
    if (sectionKey === 'search_tuning') {
      config[fieldKey] = newValue;
      return;
    }

    // General section fields are at root
    if (sectionKey === 'general') {
      config[fieldKey] = newValue;
      return;
    }

    // For other sections, update nested object with validation
    if (!config[sectionKey]) {
      config[sectionKey] = {};
    }

    const section = config[sectionKey];
    if (!section || typeof section !== 'object') {
      console.error(`Cannot update field: section ${sectionKey} is not an object`);
      return;
    }

    const sectionObj = section as Record<string, unknown>;

    // Handle flattened nested properties
    if (fieldKey.includes('_')) {
      const parts = fieldKey.split('_');
      if (parts.length === 2) {
        const [prefix, suffix] = parts;
        if (!sectionObj[prefix] || typeof sectionObj[prefix] !== 'object') {
          sectionObj[prefix] = {};
        }
        (sectionObj[prefix] as Record<string, unknown>)[suffix] = newValue;
        return;
      } else if (parts.length === 3) {
        const prefix = parts[0];
        const suffix = parts.slice(1).join('_');
        if (!sectionObj[prefix] || typeof sectionObj[prefix] !== 'object') {
          sectionObj[prefix] = {};
        }
        (sectionObj[prefix] as Record<string, unknown>)[suffix] = newValue;
        return;
      }
    }

    sectionObj[fieldKey] = newValue;
  }
</script>

{#each schema.sections as section}
  {@const filteredFields = section.fields.filter(
    (f) => f.scope === scope || f.scope === 'Both'
  )}
  {#if filteredFields.length > 0}
    <section>
      <h3>{t(section.i18n_key)}</h3>
      <div class="form-grid">
        {#each filteredFields as field}
          <DynamicField
            {field}
            {scope}
            value={getFieldValue(section.key, field.key)}
            globalValue={scope === 'Vault' ? getFieldValue(section.key, field.key, globalConfig) : undefined}
            onchange={(newValue) => updateFieldValue(section.key, field.key, newValue)}
          />
        {/each}
      </div>
    </section>
  {/if}
{/each}

<div class="actions">
  <button onclick={onsave}>{t('settings_save', 'Save')}</button>
  {#if status}<span class="status-msg">{status}</span>{/if}
</div>

<style>
  section {
    margin-bottom: 1.2rem;
  }
  section:last-child {
    margin-bottom: 0;
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
  .form-grid {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
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
</style>
