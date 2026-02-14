import type { SearchResult, VaultStatus, KajetConfig, ActionRequest, ActionResponse, DocumentListResponse, DocumentDetail } from './types';
import type { ConfigSchema } from './types/generated/ConfigSchema';

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

export async function dispatchAction(request: ActionRequest): Promise<ActionResponse> {
  const resp = await fetch('/api/actions', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(request),
  });
  if (!resp.ok) throw new Error(`Action failed: ${resp.statusText}`);
  return resp.json();
}

export async function getDocuments(
  params: { search?: string; tag?: string; limit?: number; offset?: number },
  signal?: AbortSignal
): Promise<DocumentListResponse> {
  const qs = new URLSearchParams();
  if (params.search) qs.set('search', params.search);
  if (params.tag) qs.set('tag', params.tag);
  qs.set('limit', String(params.limit ?? 50));
  qs.set('offset', String(params.offset ?? 0));
  const resp = await fetch(`/api/documents?${qs}`, { signal });
  if (!resp.ok) throw new Error(`Failed to fetch documents: ${resp.statusText}`);
  return resp.json();
}

export async function getDocumentDetail(path: string, signal?: AbortSignal): Promise<DocumentDetail> {
  const resp = await fetch(`/api/documents/${encodeURIComponent(path)}`, { signal });
  if (!resp.ok) throw new Error(`Failed to fetch document: ${resp.statusText}`);
  return resp.json();
}

export async function getConfigSchema(signal?: AbortSignal): Promise<ConfigSchema> {
  const resp = await fetch('/api/config/schema', { signal });
  if (!resp.ok) throw new Error(`Failed to fetch config schema: ${resp.statusText}`);
  return resp.json();
}

export async function getVaultConfig(signal?: AbortSignal): Promise<Record<string, unknown>> {
  const resp = await fetch('/api/config/vault', { signal });
  if (!resp.ok) throw new Error(`Failed to fetch vault config: ${resp.statusText}`);
  return resp.json();
}
