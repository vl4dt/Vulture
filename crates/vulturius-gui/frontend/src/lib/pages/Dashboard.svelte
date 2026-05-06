<script lang="ts">
  import { systemInfo, scanProgress, analysisResults, riskTolerance } from '../stores/app';
  import { invoke } from '@tauri-apps/api/core';
  import GlassPanel from '../components/GlassPanel.svelte';
  import NeonButton from '../components/NeonButton.svelte';
  import StorageOrb from '../components/StorageOrb.svelte';
  import ActivityStream from '../components/ActivityStream.svelte';

  function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
  }
</script>

<div class="space-y-6 animate-fade-in">
  <!-- Central Storage Orb -->
  <div class="flex justify-center py-8">
    <StorageOrb />
  </div>

  <!-- Quick Stats Row -->
  <div class="grid grid-cols-4 gap-4">
    <GlassPanel panelClass="p-4">
      <div class="text-xs font-mono text-fui-white/40 mb-1">SCANNED</div>
      <div class="text-2xl font-mono data-number text-fui-cyan">
        {$analysisResults.totalFiles}
      </div>
      <div class="text-xs text-fui-white/30 mt-1">files indexed</div>
    </GlassPanel>

    <GlassPanel panelClass="p-4">
      <div class="text-xs font-mono text-fui-white/40 mb-1">SAFE TO DELETE</div>
      <div class="text-2xl font-mono data-number text-fui-teal">
        {$analysisResults.safeToDelete}
      </div>
      <div class="text-xs text-fui-white/30 mt-1">
        {formatBytes($analysisResults.reclaimableSpace)}
      </div>
    </GlassPanel>

    <GlassPanel panelClass="p-4">
      <div class="text-xs font-mono text-fui-white/40 mb-1">CAUTION</div>
      <div class="text-2xl font-mono data-number text-fui-amber">
        {$analysisResults.caution}
      </div>
      <div class="text-xs text-fui-white/30 mt-1">review needed</div>
    </GlassPanel>

    <GlassPanel panelClass="p-4">
      <div class="text-xs font-mono text-fui-white/40 mb-1">PROTECTED</div>
      <div class="text-2xl font-mono data-number text-fui-crimson">
        {$analysisResults.doNotDelete}
      </div>
      <div class="text-xs text-fui-white/30 mt-1">do not touch</div>
    </GlassPanel>
  </div>

  <!-- Quick Actions -->
  <div class="flex gap-4 justify-center">
    <NeonButton color="cyan" onclick={() => import('../pages/Scanner.svelte').then(() => {})}>
      ◉ SCAN
    </NeonButton>
    <NeonButton color="teal">
      ⚡ CLEAN
    </NeonButton>
    <NeonButton color="amber">
      🔍 ANALYZE
    </NeonButton>
    <NeonButton color="purple">
      📊 REPORT
    </NeonButton>
  </div>

  <!-- Drive Info -->
  <GlassPanel panelClass="p-4 scan-line-overlay">
    <h3 class="text-xs font-mono text-fui-white/40 mb-3 tracking-widest">STORAGE MAP</h3>
    <div class="space-y-2">
      {#each $systemInfo.drives as drive}
        <div class="flex items-center gap-3">
          <div class="w-2 h-2 rounded-full bg-fui-cyan/60"></div>
          <span class="text-sm font-mono text-fui-white/70 flex-1">
            {drive.mount_point || 'Unknown'}
          </span>
          <span class="text-xs font-mono text-fui-white/40">
            {formatBytes(drive.available_space || 0)} free
          </span>
        </div>
      {/each}
      {#if $systemInfo.drives.length === 0}
        <div class="text-sm text-fui-white/30 font-mono">No drives detected</div>
      {/if}
    </div>
  </GlassPanel>

  <!-- Activity Stream -->
  <ActivityStream />
</div>
