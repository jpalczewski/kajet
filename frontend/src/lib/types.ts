export interface QueryEvent {
  query: string;
  num_results: number;
  timestamp: string;
}

export interface SearchResult {
  note_path: string;
  breadcrumb: string;
  content: string;
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

export interface KajetConfig {
  port: number;
  language: string;
  exclude_folders: string[];
  default_limit: number;
  max_concurrent_files: number;
  pipeline_buffer_size: number;
  embedding_model: string;
  open_browser: boolean;
  logging: LoggingConfig;
}

export type WsMessage =
  | { type: 'query'; data: QueryEvent }
  | { type: 'log'; data: LogEntry };
