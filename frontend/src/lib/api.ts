import type { SearchResult, VaultStatus, KajetConfig } from './types';

export async function searchVault(query: string, limit = 10): Promise<SearchResult[]> {
  const params = new URLSearchParams({ q: query, limit: String(limit) });
  const res = await fetch(`/api/search?${params}`);
  if (!res.ok) throw new Error(`Search failed: ${res.statusText}`);
  return res.json();
}

export async function getStatus(): Promise<VaultStatus> {
  const res = await fetch('/api/status');
  if (!res.ok) throw new Error(`Status failed: ${res.statusText}`);
  return res.json();
}

export async function getConfig(): Promise<KajetConfig> {
  const res = await fetch('/api/config');
  if (!res.ok) throw new Error(`Config failed: ${res.statusText}`);
  return res.json();
}

export async function updateGlobalConfig(updates: Record<string, unknown>): Promise<void> {
  const res = await fetch('/api/config/global', {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ updates }),
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(text || res.statusText);
  }
}

export async function updateVaultConfig(updates: Record<string, unknown>): Promise<void> {
  const res = await fetch('/api/config/vault', {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ updates }),
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(text || res.statusText);
  }
}
