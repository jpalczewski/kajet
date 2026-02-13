// Re-export generated types from ts-rs
export type { ActionEvent } from './types/generated/ActionEvent';
export type { ActionRequest } from './types/generated/ActionRequest';
export type { ActionResponse } from './types/generated/ActionResponse';
export type { SearchResultSummary } from './types/generated/SearchResultSummary';
export type { IndexStats } from './types/generated/IndexStats';
export type { IndexMode } from './types/generated/IndexMode';
export type { ConfigSection } from './types/generated/ConfigSection';
export type { DocumentListResponse } from './types/generated/DocumentListResponse';
export type { DocumentSummary } from './types/generated/DocumentSummary';
export type { DocumentDetail } from './types/generated/DocumentDetail';
export type { ChunkDetail } from './types/generated/ChunkDetail';
export type { Document } from './types/generated/Document';
export type { Link as GeneratedLink } from './types/generated/Link';

// Legacy query event (kept for compatibility during migration)
export interface QueryEvent {
  query: string;
  num_results: number;
  timestamp: string;
}

export interface Link {
  target: string;
  alias?: string;
  resolved_path?: string;
}

export interface SearchResult {
  note_path: string;
  breadcrumb: string;
  content: string;
  raw_content: string;
  links: Link[];
  score: number;
}

export interface VaultStatus {
  vault_path: string;
  note_count: number;
  chunk_count: number;
  model: string;
  language: string;
  indexing: boolean;
}

export interface LogEntry {
  timestamp: string;
  level: string;
  target: string;
  message: string;
  fields?: Record<string, unknown>;
}

export interface LoggingConfig {
  level: string;
  file_level: string;
  dashboard_level: string;
  progress_percent_step: number;
}

export interface TimestampConfig {
  enabled: boolean;
  created_field: string;
  modified_field: string;
  format: string;
  timezone: string;
}

export interface FrontmatterConfig {
  default_tags: string[];
  created_date_field?: string;
  modified_date_field?: string;
}

export interface WriterConfig {
  backup_enabled: boolean;
  backup_max_per_file: number;
  timestamps: TimestampConfig;
  frontmatter: FrontmatterConfig;
}

export interface TreeConfig {
  depth: number;
  size: number;
  max_chars: number;
}

export type EmbeddingBackend = 'candle' | 'remote';

export interface EmbeddingConfig {
  backend: EmbeddingBackend;
  model: string;
  base_url: string;
  api_key: string;
  document_prefix: string;
  query_prefix: string;
  remote_max_batch_size: number;
  remote_max_input_chars: number;
}

export interface KajetConfig {
  port: number;
  language: string;
  exclude_folders: string[];
  default_limit: number;
  max_concurrent_files: number;
  pipeline_buffer_size: number;
  embedding: EmbeddingConfig;
  open_browser: boolean;
  filter_overfetch_multiplier: number;
  tags_only_fetch_limit: number;
  logging: LoggingConfig;
  writer: WriterConfig;
  tree: TreeConfig;
}

// Legacy WsMessage (replaced by ActionEvent)
export type WsMessage =
  | { type: 'query'; data: QueryEvent }
  | { type: 'log'; data: LogEntry };
