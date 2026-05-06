<script lang="ts">
  // Simulated confidence scores for visualization
  let scores = $derived(Array.from({ length: 64 }, () => Math.random()));

  function scoreColor(score: number): string {
    if (score >= 0.75) return 'bg-fui-teal/40';
    if (score >= 0.5) return 'bg-fui-amber/40';
    if (score >= 0.25) return 'bg-fui-crimson/30';
    return 'bg-fui-white/5';
  }
</script>

<div class="grid grid-cols-8 gap-1">
  {#each scores as score (score)}
    <div
      class="w-full aspect-square rounded-sm transition-all duration-300 hover:scale-125 cursor-pointer {scoreColor(score)}"
      title="Confidence: {(score * 100).toFixed(0)}%"
    ></div>
  {/each}
</div>

<div class="flex justify-between mt-2 text-xs font-mono text-fui-white/30">
  <span>LOW CONFIDENCE</span>
  <span>HIGH CONFIDENCE</span>
</div>
