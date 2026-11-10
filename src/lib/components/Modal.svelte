<script lang="ts">
  import type { Snippet } from 'svelte';
  let { onClose, children, overlayClass = 'modal-overlay' }: {
    onClose: () => void;
    children: Snippet;
    overlayClass?: string;
  } = $props();
</script>

<div
  class="modal-overlay {overlayClass}"
  role="dialog"
  aria-modal="true"
  tabindex="-1"
  onclick={(event) => { if (event.target === event.currentTarget) onClose(); }}
  onkeydown={(event) => { if (event.key === 'Escape' && !event.defaultPrevented) { event.stopPropagation(); onClose(); } }}
>
  {@render children()}
</div>
