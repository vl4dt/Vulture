<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { analysisResults, riskTolerance } from '../stores/app';
  import GlassPanel from '../components/GlassPanel.svelte';
  import NeonButton from '../components/NeonButton.svelte';
  import Heatmap from '../components/Heatmap.svelte';
  import RiskDial from '../components/RiskDial.svelte';

  let analyzing = $state(false);
  let analyzeMessage = $state('');

  const riskLevels = ['paranoid', 'conservative', 'balanced', 'aggressive', 'nuclear'];

  async function runAnalysis() {
    analyzing = true;
    analyzeMessage = 'Analyzing files...';

    try {
      const result = await invoke('analyze_files', {
        request: {
          riskTolerance: $riskTolerance,
        },
      });
      analysisResults.set(result as any);
      analyzeMessage = 'Analysis complete';
    } catch (e: any) {
      analyzeMessage = `Error: ${e}`;
    } finally {
      analyzing = false;
    }
  }

  function confidenceColor(score: number): string {
    if (score >= 0.75) return 'text-fui-teal';
    if (score >= 0.5) return 'text-fui-amber';
    return 'text-fui-crimson';
  }
</script>

<div class="space-y-6 animate-fade-in">
  <!-- Risk Dial -->
  <div class="flex justify-center">
    <RiskDial />
  </div>

  <!-- Analysis Controls -->
  <GlassPanel panelClass="p-6">
    <h3 class="text-xs font-mono text-fui-white/40 mb-4 tracking-widest">ANALYSIS CONTROLS</h3>

    <div class="flex gap-4 mb-4">
      {#each riskLevels as level}
        <button
          class={`px-3 py-1 rounded text-xs font-mono transition-all fui-hover ${$riskTolerance === level ? 'bg-fui-cyan/20 text-fui-cyan' : 'text-fui-white/40'}`}
          onclick={() => riskTolerance.set(level)}
        >
          {level.toUpperCase()}
        </button>
      {/each}
    </div>

    <NeonButton color="amber" disabled={analyzing} onclick={runAnalysis}>
      {analyzing ? '⚡ ANALYZING...' : '⚡ RUN ANALYSIS'}
    </NeonButton>

    {#if analyzeMessage}
      <div class="mt-4 text-sm font-mono text-fui-white/60">{analyzeMessage}</div>
    {/if}
  </GlassPanel>

  <!-- Results Summary -->
  <div class="grid grid-cols-3 gap-4">
    <GlassPanel panelClass="p-4">
      <div class="text-xs font-mono text-fui-white/40 mb-1">SAFE TO DELETE</div>
      <div class="text-3xl font-mono data-number text-fui-teal">
        {$analysisResults.safeToDelete}
      </div>
    </GlassPanel>
    <GlassPanel panelClass="p-4">
      <div class="text-xs font-mono text-fui-white/40 mb-1">CAUTION</div>
      <div class="text-3xl font-mono data-number text-fui-amber">
        {$analysisResults.caution}
      </div>
    </GlassPanel>
    <GlassPanel panelClass="p-4">
      <div class="text-xs font-mono text-fui-white/40 mb-1">PROTECTED</div>
      <div class="text-3xl font-mono data-number text-fui-crimson">
        {$analysisResults.doNotDelete}
      </div>
    </GlassPanel>
  </div>

  <!-- Confidence Heatmap -->
  <GlassPanel panelClass="p-4 scan-line-overlay">
    <h3 class="text-xs font-mono text-fui-white/40 mb-3 tracking-widest">CONFIDENCE HEATMAP</h3>
    <Heatmap />
  </GlassPanel>
</div>
