let translations = $state<Record<string, string>>({});

async function load() {
  try {
    const res = await fetch('/api/i18n');
    translations = await res.json();
  } catch {
    // fallback: keep empty, keys used as fallback
  }
}

load();

export function t(key: string, fallback?: string): string {
  return translations[key] ?? fallback ?? key;
}

export async function reloadTranslations(): Promise<void> {
  await load();
}

export function getI18nStore() {
  return {
    get translations() { return translations; },
    t,
  };
}
