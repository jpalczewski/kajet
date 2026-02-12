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

export type WsMessage =
  | { type: 'query'; data: QueryEvent }
  | { type: 'log'; data: LogEntry };
