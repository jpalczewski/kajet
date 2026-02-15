<script lang="ts">
  import { page } from '$app/state';

  let { href, label }: { href: string; label: string } = $props();

  let isActive = $derived(
    href === '/'
      ? page.url.pathname === '/'
      : page.url.pathname === href || page.url.pathname.startsWith(href + '/')
  );
</script>

<a {href} class:active={isActive}>{label}</a>

<style>
  a {
    color: #888;
    text-decoration: none;
    font-size: 0.8rem;
    padding: 0.3rem 0.6rem;
    border-radius: 4px;
    transition: color 0.15s, background 0.15s;
  }
  a:hover { color: #ccc; background: #1a1a1a; }
  a.active { color: #fff; background: #222; }
</style>
