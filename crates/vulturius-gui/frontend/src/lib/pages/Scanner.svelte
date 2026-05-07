<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { scanProgress } from '../stores/app';
  import GlassPanel from '../components/GlassPanel.svelte';
  import NeonButton from '../components/NeonButton.svelte';
  import RadarSweep from '../components/RadarSweep.svelte';

  let scanning = $state(false);
  let scanType = $state<'full' | 'quick' | 'custom'>('full');
  let scanMessage = $state('');
  let targetPath = $state('');
  let minFileSizeMb = $state(0); // 0 = no limit
  let selectedTypes = $state<string[]>([]);

  const allFileTypes = [
    { id: 'temp', label: 'Temp' },
    { id: 'cache', label: 'Cache' },
    { id: 'log', label: 'Logs' },
    { id: 'crashdump', label: 'Crash Dumps' },
    { id: 'thumbnail', label: 'Thumbnails' },
    { id: 'download', label: 'Downloads' },
    { id: 'package', label: 'Package Cache' },
    { id: 'browser', label: 'Browser Data' },
    { id: 'backup', label: 'Backups' },
    { id: 'duplicate', label: 'Duplicates' },
  ];

  function toggleType(id: string) {
    if (selectedTypes.includes(id)) {
      selectedTypes = selectedTypes.filter(t => t !== id);
    } else {
      selectedTypes = [...selectedTypes, id];
    }
  }

  // Listen for scan progress events
  $effect(() => {
    listen('scan-progress', (event) => {
      const data = event.payload as { filesScanned: number; dirsScanned: number; totalSize: number; message: string };
      scanProgress.set({
        inProgress: true,
        filesScanned: data.filesScanned,
        dirsScanned: data.dirsScanned,
        totalSize: data.totalSize,
        percentage: Math.min(99, (data.filesScanned / 1000) * 100),
        message: data.message,
      });
      scanMessage = data.message;
    });

    listen('scan-complete', (event) => {
      const data = event.payload as { scanId: string; totalFiles: number; totalSize: number };
      scanProgress.set({
        inProgress: false,
        filesScanned: data.totalFiles,
        dirsScanned: 0,
        totalSize: data.totalSize,
        percentage: 100,
        message: `Scan complete. ID: ${data.scanId}`,
      });
      scanMessage = `Scan complete. ${data.totalFiles} files, ${(data.totalSize / 1024 / 1024).toFixed(1)} MB. ID: ${data.scanId}`;
    });
  });

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
          targetPath: targetPath || undefined,
          fileTypes: selectedTypes.length > 0 ? selectedTypes : undefined,
          minFileSizeMb: minFileSizeMb > 0 ? minFileSizeMb : undefined,
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

  async function stopScan() {
    try {
      await invoke('cancel_scan');
      scanning = false;
      scanMessage = 'Scan cancelled';
      scanProgress.update(s => ({ ...s, inProgress: false, message: 'Cancelled' }));
    } catch (e: any) {
      scanMessage = `Cancel failed: ${e}`;
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

    <!-- Scan Type -->
    <div class="flex gap-4 mb-4">
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
      <button
        class={`px-4 py-2 rounded-lg font-mono text-sm transition-all fui-hover ${scanType === 'custom' ? 'bg-fui-cyan/20 text-fui-cyan' : 'text-fui-white/40'}`}
        onclick={() => scanType = 'custom'}
      >
        CUSTOM
      </button>
    </div>

    <!-- Custom Options -->
    {#if scanType === 'custom'}
      <!-- Target Path -->
      <div class="mb-4">
        <label class="text-xs font-mono text-fui-white/40 block mb-1">TARGET PATH (leave empty for full scan)</label>
        <input
          type="text"
          bind:value={targetPath}
          placeholder="e.g. C:\Users\vl4dt\Downloads or C:\Windows\Temp"
          class="w-full px-3 py-2 rounded-lg font-mono text-sm bg-fui-panel border border-fui-cyan/10 text-fui-white/70 placeholder:text-fui-white/20"
        />
      </div>

      <!-- Min File Size -->
      <div class="mb-4">
        <label class="text-xs font-mono text-fui-white/40 block mb-1">MIN FILE SIZE (MB, 0 = no limit)</label>
        <input
          type="number"
          bind:value={minFileSizeMb}
          min="0"
          step="1"
          class="w-full px-3 py-2 rounded-lg font-mono text-sm bg-fui-panel border border-fui-cyan/10 text-fui-white/70"
        />
      </div>

      <!-- File Type Filter -->
      <div class="mb-4">
        <label class="text-xs font-mono text-fui-white/40 block mb-2">FILE TYPES (leave empty for all)</label>
        <div class="grid grid-cols-2 gap-2">
          {#each allFileTypes as ft}
            <label class="flex items-center gap-2 cursor-pointer text-xs font-mono text-fui-white/60">
              <input
                type="checkbox"
                checked={selectedTypes.includes(ft.id)}
                onchange={() => toggleType(ft.id)}
                class="accent-fui-cyan"
              />
              {ft.label}
            </label>
          {/each}
        </div>
      </div>
    {/if}

    <!-- Action Buttons -->
    <div class="flex gap-3">
      <NeonButton color="cyan" disabled={scanning} onclick={startScan} class="flex-1">
        {scanning ? '◉ SCANNING...' : '◉ START SCAN'}
      </NeonButton>
      <NeonButton color="crimson" disabled={!scanning} onclick={stopScan} class="flex-1">
        ✕ STOP
      </NeonButton>
    </div>

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
