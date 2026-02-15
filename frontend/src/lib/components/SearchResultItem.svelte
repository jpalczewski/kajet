<script lang="ts">
  import type { SearchResult } from '$lib/types';

  let { result }: { result: SearchResult } = $props();
</script>

<div class="chunk">
  <div class="breadcrumb">{result.breadcrumb}</div>
  <div class="content">{result.content}</div>
  <span class="score">{result.score.toFixed(4)}</span>
</div>

<style>
  .chunk {
    background: #161616;
    padding: 0.6rem 0.8rem;
    margin: 0.4rem 0;
    border-radius: 6px;
    border-left: 3px solid #333;
  }
  .breadcrumb { color: #777; font-size: 0.7rem; margin-bottom: 0.3rem; }
  .content {
    font-size: 0.8rem;
    line-height: 1.5;
    color: #bbb;
    white-space: pre-wrap;
    max-height: 6rem;
    overflow: hidden;
  }
  .score {
    display: inline-block;
    margin-top: 0.3rem;
    color: #5a5;
    font-size: 0.7rem;
    font-weight: 600;
  }
</style>
