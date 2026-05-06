<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import GlassPanel from '../components/GlassPanel.svelte';
  import NeonButton from '../components/NeonButton.svelte';
  import OrbitRing from '../components/OrbitRing.svelte';

  let apps = $state<any[]>([]);
  let loading = $state(false);
  let filter = $state<'all' | 'unused' | 'active'>('all');

  async function loadApps() {
    loading = true;
    try {
      apps = await invoke('get_apps');
    } catch (e: any) {
      console.error('Failed to load apps:', e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    loadApps();
  });

  let filteredApps = $derived(apps.filter(app => {
    if (filter === 'all') return true;
    if (filter === 'unused') return app.recommendation === 'unused' || app.recommendation === 'rarely-used';
    if (filter === 'active') return app.recommendation === 'active' || app.recommendation === 'essential';
    return true;
  }));

  function recommendationColor(rec: string): string {
    switch (rec) {
      case 'unused': return 'text-fui-crimson';
      case 'rarely-used': return 'text-fui-amber';
      case 'active': return 'text-fui-teal';
      case 'essential': return 'text-fui-cyan';
      default: return 'text-fui-white/60';
    }
  }
</script>

<div class="space-y-6 animate-fade-in">
  <!-- App Usage Orbit Rings -->
  <div class="flex justify-center py-4">
    <OrbitRing />
  </div>

  <!-- Filters -->
  <GlassPanel panelClass="p-4">
    <div class="flex gap-3">
      {#each ['all', 'unused', 'active'] as f}
        <button
          class={`px-4 py-2 rounded-lg font-mono text-xs transition-all fui-hover ${filter === f ? 'bg-fui-cyan/20 text-fui-cyan' : 'text-fui-white/40'}`}
          onclick={() => filter = f as any}
        >
          {f.toUpperCase()}
        </button>
      {/each}
    </div>
  </GlassPanel>

  <!-- App List -->
  <GlassPanel panelClass="p-4 scan-line-overlay">
    <h3 class="text-xs font-mono text-fui-white/40 mb-3 tracking-widest">
      INSTALLED APPLICATIONS ({filteredApps.length})
    </h3>

    {#if loading}
      <div class="text-sm font-mono text-fui-white/40 text-center py-8">Loading...</div>
    {:else if filteredApps.length === 0}
      <div class="text-sm font-mono text-fui-white/40 text-center py-8">No applications found</div>
    {:else}
      <div class="space-y-2 max-h-96 overflow-auto">
        {#each filteredApps as app (app.id)}
          <div class="fui-glass p-3 flex items-center gap-4 fui-hover transition-all">
            <!-- Usage indicator -->
            <div class="w-2 h-2 rounded-full" class:bg-fui-teal={app.usage_score > 0.5} class:bg-fui-amber={app.usage_score > 0.2 && app.usage_score <= 0.5} class:bg-fui-crimson={app.usage_score <= 0.2}></div>

            <!-- App info -->
            <div class="flex-1 min-w-0">
              <div class="text-sm font-mono text-fui-white/80 truncate">{app.name}</div>
              <div class="text-xs font-mono text-fui-white/30">
                {app.version || 'v?'} · {app.recommendation}
              </div>
            </div>

            <!-- Usage score -->
            <div class="text-right">
              <div class="text-xs font-mono data-number {recommendationColor(app.recommendation)}">
                {(app.usage_score * 100).toFixed(0)}%
              </div>
              <div class="text-xs font-mono text-fui-white/30">usage</div>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </GlassPanel>
</div>
