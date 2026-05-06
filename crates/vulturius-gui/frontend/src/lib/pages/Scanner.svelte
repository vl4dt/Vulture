<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { scanProgress } from '../stores/app';
  import GlassPanel from '../components/GlassPanel.svelte';
  import NeonButton from '../components/NeonButton.svelte';
  import RadarSweep from '../components/RadarSweep.svelte';

  let scanning = $state(false);
  let scanType = $state<'quick' | 'full'>('full');
  let scanMessage = $state('');

  async function startScan() {
    scanning = true;
    scanMessage = 'Initializing scan...';
    scanProgress.set({
      inProgress: true,
      filesScanned: 0,
      dirsScanned: 0,
      totalSize: 0,
      percentage: 0,
      message: 'Scanning...',
    });

    try {
      const result = await invoke('start_scan', {
        request: {
          scanType,
          quick: scanType === 'quick',
        },
      });
      scanMessage = `Scan complete. ID: ${result}`;
    } catch (e: any) {
      scanMessage = `Error: ${e}`;
    } finally {
      scanning = false;
      scanProgress.update(s => ({ ...s, inProgress: false }));
    }
  }
</script>

<div class="space-y-6 animate-fade-in">
  <!-- Radar Sweep Visualization -->
  <div class="flex justify-center py-4">
    <RadarSweep {scanning} />
  </div>

  <!-- Scan Controls -->
  <GlassPanel panelClass="p-6">
    <h3 class="text-xs font-mono text-fui-white/40 mb-4 tracking-widest">SCAN CONFIGURATION</h3>

    <div class="flex gap-4 mb-6">
      <button
        class={`px-4 py-2 rounded-lg font-mono text-sm transition-all fui-hover ${scanType === 'full' ? 'bg-fui-cyan/20 text-fui-cyan' : 'text-fui-white/40'}`}
        onclick={() => scanType = 'full'}
      >
        FULL SCAN
      </button>
      <button
        class={`px-4 py-2 rounded-lg font-mono text-sm transition-all fui-hover ${scanType === 'quick' ? 'bg-fui-cyan/20 text-fui-cyan' : 'text-fui-white/40'}`}
        onclick={() => scanType = 'quick'}
      >
        QUICK SCAN
      </button>
    </div>

    <NeonButton color="cyan" disabled={scanning} onclick={startScan}>
      {scanning ? '◉ SCANNING...' : '◉ START SCAN'}
    </NeonButton>

    {#if scanMessage}
      <div class="mt-4 text-sm font-mono text-fui-white/60">{scanMessage}</div>
    {/if}
  </GlassPanel>

  <!-- Live Progress -->
  {#if $scanProgress.inProgress}
    <GlassPanel panelClass="p-6">
      <h3 class="text-xs font-mono text-fui-white/40 mb-4 tracking-widest">LIVE PROGRESS</h3>

      <div class="space-y-3">
        <div class="flex justify-between text-sm font-mono">
          <span class="text-fui-white/60">Files scanned</span>
          <span class="text-fui-cyan data-number">{$scanProgress.filesScanned}</span>
        </div>
        <div class="flex justify-between text-sm font-mono">
          <span class="text-fui-white/60">Directories</span>
          <span class="text-fui-cyan data-number">{$scanProgress.dirsScanned}</span>
        </div>
        <div class="flex justify-between text-sm font-mono">
          <span class="text-fui-white/60">Total size</span>
          <span class="text-fui-cyan data-number">
            {($scanProgress.totalSize / 1024 / 1024).toFixed(1)} MB
          </span>
        </div>

        <!-- Progress Bar -->
        <div class="mt-4 h-1 bg-white/5 rounded-full overflow-hidden">
          <div
            class="h-full bg-fui-cyan transition-all duration-300 rounded-full"
            style="width: {$scanProgress.percentage}%"
          ></div>
        </div>
      </div>
    </GlassPanel>
  {/if}
</div>
