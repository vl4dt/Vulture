<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { cleanupStatus, riskTolerance } from '../stores/app';
  import GlassPanel from '../components/GlassPanel.svelte';
  import NeonButton from '../components/NeonButton.svelte';
  import ParticleFX from '../components/ParticleFX.svelte';

  let cleaning = $state(false);
  let dryRun = $state(true);
  let quarantine = $state(true);
  let cleanMessage = $state('');

  async function executeClean() {
    cleaning = true;
    cleanMessage = dryRun ? 'Dry run: simulating cleanup...' : 'Cleaning files...';
    cleanupStatus.set({
      inProgress: true,
      itemsProcessed: 0,
      itemsSucceeded: 0,
      itemsFailed: 0,
      spaceFreed: 0,
    });

    try {
      const result = await invoke('execute_cleanup', {
        request: {
          dryRun,
          quarantine,
          riskTolerance: $riskTolerance,
        },
      });
      cleanupStatus.set({
        inProgress: false,
        ...result,
      } as any);
      cleanMessage = dryRun
        ? `Dry run complete. Would free ${formatBytes(result.space_freed || 0)}`
        : `Cleanup complete. Freed ${formatBytes(result.space_freed || 0)}`;
    } catch (e: any) {
      cleanMessage = `Error: ${e}`;
    } finally {
      cleaning = false;
    }
  }

  function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
  }
</script>

<div class="space-y-6 animate-fade-in">
  <!-- Particle Destruction FX -->
  {#if cleaning}
    <div class="flex justify-center">
      <ParticleFX active={true} />
    </div>
  {/if}

  <!-- Cleanup Controls -->
  <GlassPanel class="p-6">
    <h3 class="text-xs font-mono text-fui-white/40 mb-4 tracking-widest">CLEANUP CONTROLS</h3>

    <div class="space-y-4 mb-6">
      <!-- Dry Run Toggle -->
      <label class="flex items-center gap-3 cursor-pointer">
        <input
          type="checkbox"
          checked={dryRun}
          onchange={(e) => dryRun = e.currentTarget.checked}
          class="sr-only peer"
        />
        <div class="w-10 h-5 bg-white/10 rounded-full peer-checked:bg-fui-cyan/30 relative transition-colors">
          <div class="w-3 h-3 bg-fui-white/60 rounded-full absolute top-1 left-1 peer-checked:left-6 transition-all" />
        </div>
        <span class="text-sm font-mono text-fui-white/60">DRY RUN MODE</span>
      </label>

      <!-- Quarantine Toggle -->
      <label class="flex items-center gap-3 cursor-pointer">
        <input
          type="checkbox"
          checked={quarantine}
          onchange={(e) => quarantine = e.currentTarget.checked}
          class="sr-only peer"
        />
        <div class="w-10 h-5 bg-white/10 rounded-full peer-checked:bg-fui-cyan/30 relative transition-colors">
          <div class="w-3 h-3 bg-fui-white/60 rounded-full absolute top-1 left-1 peer-checked:left-6 transition-all" />
        </div>
        <span class="text-sm font-mono text-fui-white/60">QUARANTINE (safe delete)</span>
      </label>
    </div>

    <NeonButton color="crimson" disabled={cleaning} onclick={executeClean}>
      {cleaning ? '✕ CLEANING...' : (dryRun ? '✕ DRY RUN' : '✕ EXECUTE CLEANUP')}
    </NeonButton>

    {#if cleanMessage}
      <div class="mt-4 text-sm font-mono text-fui-white/60">{cleanMessage}</div>
    {/if}
  </GlassPanel>

  <!-- Cleanup Results -->
  {#if !cleaning && cleanupStatus.itemsProcessed > 0}
    <GlassPanel class="p-6">
      <h3 class="text-xs font-mono text-fui-white/40 mb-4 tracking-widest">RESULTS</h3>

      <div class="grid grid-cols-4 gap-4">
        <div>
          <div class="text-xs font-mono text-fui-white/40">PROCESSED</div>
          <div class="text-xl font-mono data-number text-fui-cyan">{cleanupStatus.itemsProcessed}</div>
        </div>
        <div>
          <div class="text-xs font-mono text-fui-white/40">SUCCEEDED</div>
          <div class="text-xl font-mono data-number text-fui-teal">{cleanupStatus.itemsSucceeded}</div>
        </div>
        <div>
          <div class="text-xs font-mono text-fui-white/40">FAILED</div>
          <div class="text-xl font-mono data-number text-fui-crimson">{cleanupStatus.itemsFailed}</div>
        </div>
        <div>
          <div class="text-xs font-mono text-fui-white/40">FREED</div>
          <div class="text-xl font-mono data-number text-fui-amber">{formatBytes(cleanupStatus.spaceFreed)}</div>
        </div>
      </div>
    </GlassPanel>
  {/if}
</div>
