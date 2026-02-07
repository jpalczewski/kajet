<script lang="ts">
  import Router from 'svelte-spa-router';
  import NavLink from './components/NavLink.svelte';
  import { getWsStore } from './lib/stores/websocket.svelte';
  import { t } from './lib/stores/i18n.svelte';

  import Dashboard from './routes/Dashboard.svelte';
  import Status from './routes/Status.svelte';
  import Settings from './routes/Settings.svelte';

  const ws = getWsStore();

  const routes = {
    '/': Dashboard,
    '/status': Status,
    '/settings': Settings,
  };
</script>

<header>
  <h1>{t('title', 'kajet')}</h1>
  <span class="subtitle">{t('subtitle', 'obsidian vault search')}</span>
  <nav>
    <NavLink href="/" label={t('nav_dashboard', 'Dashboard')} />
    <NavLink href="/status" label={t('nav_status', 'Status')} />
    <NavLink href="/settings" label={t('nav_settings', 'Settings')} />
  </nav>
  <span class="status" class:disconnected={!ws.connected}>
    {ws.connected ? t('ws_connected', '● connected') : t('ws_disconnected', '● disconnected')}
  </span>
</header>

<main>
  <Router {routes} />
</main>

<style>
  :global(*) { margin: 0; box-sizing: border-box; }
  :global(body) {
    font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', monospace;
    background: #0a0a0a;
    color: #c8c8c8;
    padding: 1.5rem;
    max-width: 1400px;
    margin: 0 auto;
  }

  header {
    display: flex;
    align-items: baseline;
    gap: 1rem;
    margin-bottom: 1.5rem;
    flex-wrap: wrap;
  }
  h1 { font-size: 1.4rem; color: #fff; }
  .subtitle { color: #555; font-size: 0.8rem; }

  nav { display: flex; gap: 0.25rem; }

  .status { color: #4a4; font-size: 0.75rem; margin-left: auto; }
  .status.disconnected { color: #a44; }
</style>
