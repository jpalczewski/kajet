<script lang="ts">
  import { updateGlobalConfig } from '../lib/api';
  import { reloadTranslations } from '../lib/stores/i18n.svelte';

  let current = $state('en');
  let loading = $state(false);

  // Load current language from config on mount
  $effect(() => {
    fetch('/api/config')
      .then((r) => r.json())
      .then((c) => (current = c.language))
      .catch(() => {});
  });

  async function onChange(e: Event) {
    const val = (e.target as HTMLSelectElement).value;
    loading = true;
    try {
      await updateGlobalConfig({ language: val });
      current = val;
      await reloadTranslations();
    } catch {
      // revert on error
    }
    loading = false;
  }
</script>

<select class="lang-select" value={current} onchange={onChange} disabled={loading}>
  <option value="en">EN</option>
  <option value="pl">PL</option>
</select>

<style>
  .lang-select {
    background: #161616;
    border: 1px solid #333;
    border-radius: 4px;
    color: #888;
    font-family: inherit;
    font-size: 0.7rem;
    padding: 0.15rem 0.3rem;
    cursor: pointer;
  }
  .lang-select:hover {
    border-color: #7af;
    color: #ccc;
  }
</style>
