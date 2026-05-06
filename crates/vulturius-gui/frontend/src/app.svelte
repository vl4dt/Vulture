<script lang="ts">
  console.log('Vulturius: app.svelte loaded');
  import Dashboard from './lib/pages/Dashboard.svelte';
  import Scanner from './lib/pages/Scanner.svelte';
  import Analyzer from './lib/pages/Analyzer.svelte';
  import Cleaner from './lib/pages/Cleaner.svelte';
  import Apps from './lib/pages/Apps.svelte';
  import Settings from './lib/pages/Settings.svelte';
  import NavRing from './lib/components/NavRing.svelte';
  import { currentPage, systemInfo, scanProgress } from './lib/stores/app';

  $effect(() => {
    // Load system info on mount (only in Tauri)
    import('@tauri-apps/api/core').then((core) => {
      if (typeof core.invoke === 'function') {
        core.invoke('get_system_info')
          .then((info) => {
            systemInfo.update(s => ({ ...s, ...info }));
          })
          .catch((err) => {
            console.error('Failed to load system info:', err);
          });
      }
    }).catch(() => {
      console.log('Not running in Tauri, skipping system info');
    });
  });
</script>

<div class="ambient-bg w-full h-screen flex overflow-hidden">
  <!-- Navigation Ring -->
  <NavRing />

  <!-- Main Content Area -->
  <main class="flex-1 flex flex-col p-6 overflow-hidden relative">
    <!-- Top Bar -->
    <header class="flex items-center justify-between mb-6 animate-fade-in">
      <div class="flex items-center gap-3">
        <div class="w-2 h-2 rounded-full bg-fui-cyan animate-pulse"></div>
        <h1 class="text-lg font-mono tracking-widest text-fui-cyan glow-text-cyan">
          VULTURIUS
        </h1>
      </div>
      <div class="flex items-center gap-4 text-xs font-mono text-fui-white/40">
        <span>{systemInfo.hostname || '...'}</span>
        <span class="w-px h-3 bg-fui-white/10"></span>
        <span>{systemInfo.os || '...'}/{systemInfo.arch || '...'}</span>
      </div>
    </header>

    <!-- Page Content -->
    <div class="flex-1 overflow-auto">
      {#if $currentPage === 'dashboard'}
        <Dashboard />
      {:else if $currentPage === 'scanner'}
        <Scanner />
      {:else if $currentPage === 'analyzer'}
        <Analyzer />
      {:else if $currentPage === 'cleaner'}
        <Cleaner />
      {:else if $currentPage === 'apps'}
        <Apps />
      {:else if $currentPage === 'settings'}
        <Settings />
      {/if}
    </div>
  </main>
</div>
