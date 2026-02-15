import type { ActionEvent } from '../types';

const MAX_QUERIES = 50;
const MAX_LOGS = 500;

// Store all ActionEvents by type
let logs = $state<ActionEvent[]>([]);
let queries = $state<ActionEvent[]>([]);
let indexProgress = $state<{ action_id: string; processed: number; total: number } | null>(null);
let isIndexing = $state(false);
let stats = $state<{ note_count: number; chunk_count: number } | null>(null);
let connected = $state(false);
let ws: WebSocket | null = null;

function connect() {
  const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
  ws = new WebSocket(`${protocol}//${location.host}/ws`);

  ws.onopen = () => {
    connected = true;
  };

  ws.onclose = () => {
    connected = false;
    setTimeout(connect, 2000);
  };

  ws.onmessage = (e) => {
    const event: ActionEvent = JSON.parse(e.data);

    switch (event.type) {
      case 'LogEntry':
        logs = [...logs, event].slice(-MAX_LOGS);
        break;

      case 'QueryExecuted':
        queries = [event, ...queries].slice(0, MAX_QUERIES);
        break;

      case 'IndexStarted':
        isIndexing = true;
        indexProgress = null;
        break;

      case 'IndexProgress':
        indexProgress = event.data;
        break;

      case 'IndexCompleted':
        isIndexing = false;
        indexProgress = null;
        if (event.data.stats) {
          stats = {
            note_count: event.data.stats.total_documents,
            chunk_count: event.data.stats.total_chunks,
          };
        }
        break;

      case 'IndexFailed':
        isIndexing = false;
        indexProgress = null;
        break;

      case 'StatsUpdated':
        stats = event.data;
        break;

      case 'ConfigChanged':
      case 'EmbedderSwapped':
        // Future: could track these for UI updates
        break;
    }
  };
}

connect();

export function getWsStore() {
  return {
    get logs() {
      return logs;
    },
    get queries() {
      return queries;
    },
    get indexProgress() {
      return indexProgress;
    },
    get isIndexing() {
      return isIndexing;
    },
    get stats() {
      return stats;
    },
    get connected() {
      return connected;
    },
  };
}
