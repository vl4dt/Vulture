<script lang="ts">
  let { panelClass = '', children } = $props();
</script>

<div class="fui-glass {panelClass}">
  {@render children()}
</div>
