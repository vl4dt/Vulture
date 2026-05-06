<script lang="ts">
  let { active = false } = $props();

  $effect(() => {
    if (!active) return;
    // Particles are handled via CSS animations
  });
</script>

{#if active}
  <div class="relative w-48 h-48">
    <!-- Particle burst effect -->
    {#each Array.from({ length: 12 }) as _, i}
      <div
        class="absolute w-1 h-1 rounded-full bg-fui-crimson animate-ping"
        style="top: 50%; left: 50%; animation-delay: {i * 0.1}s; animation-duration: {1 + Math.random()}s;"
      ></div>
    {/each}

    <!-- Dissolving ring -->
    <div class="absolute inset-4 rounded-full border-2 border-fui-crimson/30 animate-pulse"></div>
    <div class="absolute inset-8 rounded-full border border-fui-crimson/20 animate-pulse" style="animation-delay: 0.3s;"></div>
    <div class="absolute inset-12 rounded-full border border-fui-crimson/10 animate-pulse" style="animation-delay: 0.6s;"></div>

    <!-- Center text -->
    <div class="absolute inset-0 flex items-center justify-center">
      <div class="text-sm font-mono text-fui-crimson glow-text-cyan animate-pulse">
        CLEANING
      </div>
    </div>
  </div>
{/if}
