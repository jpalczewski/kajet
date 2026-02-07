import type { QueryEvent } from '../types';

const MAX_EVENTS = 50;

let events = $state<QueryEvent[]>([]);
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
    const event: QueryEvent = JSON.parse(e.data);
    events = [event, ...events].slice(0, MAX_EVENTS);
  };
}

connect();

export function getWsStore() {
  return {
    get events() { return events; },
    get connected() { return connected; },
  };
}
