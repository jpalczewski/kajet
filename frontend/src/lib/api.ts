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
