<script lang="ts">
  export let scanning: boolean = false;
</script>

<div class="relative w-48 h-48">
  <!-- Radar circles -->
  <div class="absolute inset-0 rounded-full border border-fui-cyan/10" />
  <div class="absolute inset-6 rounded-full border border-fui-cyan/8" />
  <div class="absolute inset-12 rounded-full border border-fui-cyan/6" />
  <div class="absolute inset-20 rounded-full border border-fui-cyan/4" />

  <!-- Cross lines -->
  <div class="absolute top-0 bottom-0 left-1/2 w-px bg-fui-cyan/5" />
  <div class="absolute left-0 right-0 top-1/2 h-px bg-fui-cyan/5" />

  <!-- Sweep animation -->
  {#if scanning}
    <div class="absolute inset-0 animate-spin-slow" style="animation-duration: 2s;">
      <div
        class="w-1/2 h-1/2 origin-bottom-right"
        style="background: conic-gradient(from 0deg, transparent 0deg, rgba(0, 240, 255, 0.15) 60deg, transparent 60deg);"
      />
    </div>
  {/if}

  <!-- Center dot -->
  <div class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-2 h-2 rounded-full bg-fui-cyan" class:animate-pulse={scanning} />

  <!-- Data points -->
  {#if scanning}
    <div class="absolute top-8 right-12 w-1 h-1 rounded-full bg-fui-cyan animate-ping" />
    <div class="absolute bottom-16 left-10 w-1 h-1 rounded-full bg-fui-teal animate-ping" style="animation-delay: 0.5s;" />
    <div class="absolute top-20 left-16 w-1 h-1 rounded-full bg-fui-cyan animate-ping" style="animation-delay: 1s;" />
  {/if}
</div>
