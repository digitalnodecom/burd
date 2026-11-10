<script lang="ts">
  import "../app.css";
  import { dialogs } from "$lib/actions/dialogs";
  let { children } = $props();
</script>

<div use:dialogs style="display: contents">{@render children()}</div>
