<script lang="ts">
  import type { SchemaField } from '$lib/types/generated/SchemaField';
  import { t } from '$lib/stores/i18n.svelte';

  interface Props {
    field: SchemaField;
    value: unknown;
    globalValue?: unknown;
    scope?: string;
    onchange: (newValue: unknown) => void;
  }

  let { field, value = $bindable(), globalValue, scope, onchange }: Props = $props();

  // Pole jest override'owane jeśli scope === "Vault" i wartość nie jest pusta
  let isOverridden = $derived(
    scope === 'Vault' &&
    value !== null &&
    value !== undefined &&
    value !== '' &&
    (Array.isArray(value) ? value.length > 0 : true)
  );

  // Format global value dla tooltip
  let globalValueDisplay = $derived(
    globalValue === undefined || globalValue === null ? '' :
    Array.isArray(globalValue) ? globalValue.join(', ') :
    typeof globalValue === 'boolean' ? (globalValue ? 'true' : 'false') :
    String(globalValue)
  );

  // Helper to get display value
  function getDisplayValue(): string | number {
    if (field.field_type.type === 'Array') {
      return Array.isArray(value) ? value.join(', ') : '';
    }
    if (field.field_type.type === 'Bool') {
      return '';  // Not used for checkboxes
    }
    return value as string | number;
  }

  // Helper to parse input value based on field type
  function handleInput(e: Event) {
    const target = e.target as HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement;
    let newValue: unknown;

    if (field.field_type.type === 'Bool') {
      newValue = (target as HTMLInputElement).checked;
    } else if (field.field_type.type === 'Number') {
      newValue = Number(target.value);
    } else if (field.field_type.type === 'Array') {
      // Parse comma-separated string to array
      const str = target.value.trim();
      newValue = str ? str.split(',').map((s) => s.trim()).filter(Boolean) : [];
    } else {
      newValue = target.value;
    }

    onchange(newValue);
  }

  function handleReset() {
    if (field.field_type.type === 'Array') {
      onchange([]);
    } else {
      onchange(null);
    }
  }

  function getInputType(): string {
    if (field.widget === 'password') return 'password';
    if (field.field_type.type === 'Number') return 'number';
    return 'text';
  }
</script>

<label class:checkbox-label={field.field_type.type === 'Bool'}>
  <span class="label">
    {t(field.i18n_key)}
    {#if isOverridden}
      <span class="badge override" title="Global: {globalValueDisplay}">
        🔸 {t('settings_override', 'Override')}
      </span>
    {/if}
    {#if field.restart_required}
      <span class="badge restart">⚠️ {t('settings_restart_required', 'Restart required')}</span>
    {:else if field.hot_swap}
      <span class="badge hot-swap">🔄 Hot-swap</span>
    {/if}
  </span>

  {#if field.field_type.type === 'Enum' && field.field_type.data}
    <select value={getDisplayValue()} oninput={handleInput}>
      {#each field.field_type.data.options as option}
        <option value={option}>{option}</option>
      {/each}
    </select>
  {:else if field.field_type.type === 'Bool'}
    <input type="checkbox" checked={value as boolean} onchange={handleInput} />
  {:else if field.widget === 'textarea'}
    <textarea value={getDisplayValue()} oninput={handleInput}></textarea>
  {:else}
    <input
      type={getInputType()}
      value={getDisplayValue()}
      oninput={handleInput}
      min={field.constraints?.min ?? undefined}
      max={field.constraints?.max ?? undefined}
    />
  {/if}

  {#if isOverridden}
    <button type="button" class="reset-btn" onclick={handleReset}>
      {t('settings_reset', 'Reset')}
    </button>
  {/if}
</label>

<style>
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
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .badge {
    font-size: 0.6rem;
    padding: 0.1rem 0.3rem;
    border-radius: 3px;
    font-weight: normal;
  }
  .badge.restart {
    background: #533;
    color: #faa;
  }
  .badge.hot-swap {
    background: #353;
    color: #afa;
  }
  .badge.override {
    background: #442;
    color: #fa7;
    cursor: help;
  }
  .reset-btn {
    background: transparent;
    border: none;
    color: #888;
    font-size: 0.7rem;
    cursor: pointer;
    padding: 0.2rem 0.4rem;
    text-decoration: underline;
    align-self: flex-start;
  }
  .reset-btn:hover {
    color: #fa7;
  }
  input[type='text'],
  input[type='number'],
  input[type='password'],
  select,
  textarea {
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
  input[type='password']:focus,
  select:focus,
  textarea:focus {
    border-color: #7af;
    outline: none;
  }
  input[type='checkbox'] {
    accent-color: #7af;
  }
  textarea {
    min-height: 4rem;
    resize: vertical;
  }
</style>
