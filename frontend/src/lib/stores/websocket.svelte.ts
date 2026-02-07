import type { QueryEvent, LogEntry, WsMessage } from '../types';

const MAX_EVENTS = 50;
const MAX_LOGS = 500;

let events = $state<QueryEvent[]>([]);
let logs = $state<LogEntry[]>([]);
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
    const msg: WsMessage = JSON.parse(e.data);
    if (msg.type === 'query') {
      events = [msg.data, ...events].slice(0, MAX_EVENTS);
    } else if (msg.type === 'log') {
      logs = [...logs, msg.data].slice(-MAX_LOGS);
    }
  };
}

connect();

export function getWsStore() {
  return {
    get events() {
      return events;
    },
    get logs() {
      return logs;
    },
    get connected() {
      return connected;
    },
  };
}
