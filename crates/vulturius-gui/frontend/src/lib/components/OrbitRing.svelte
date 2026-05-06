<script lang="ts">
  // Simulated app usage data for visualization
  const orbitData = [
    { label: 'Active', count: 12, color: 'bg-fui-teal', radius: 60 },
    { label: 'Rarely Used', count: 5, color: 'bg-fui-amber', radius: 45 },
    { label: 'Unused', count: 3, color: 'bg-fui-crimson', radius: 30 },
  ];
</script>

<div class="relative w-48 h-48">
  <!-- Center -->
  <div class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 text-center">
    <div class="text-2xl font-mono data-number text-fui-cyan">{orbitData.reduce((s, d) => s + d.count, 0)}</div>
    <div class="text-xs font-mono text-fui-white/30">APPS</div>
  </div>

  <!-- Orbit rings -->
  {#each orbitData as data}
    <div
      class="absolute rounded-full border border-fui-white/5"
      style="width: {data.radius * 2}%; height: {data.radius * 2}%; top: calc(50% - {data.radius}%); left: calc(50% - {data.radius}%);"
    ></div>
    <!-- Orbiting dots -->
    {#each Array.from({ length: data.count }) as _, i}
      <div
        class="absolute w-2 h-2 rounded-full {data.color} animate-spin-slow"
        style="animation-duration: {4 + i * 0.5}s; top: calc(50% - {data.radius}%); left: 50%;"
      ></div>
    {/each}
  {/each}
</div>
