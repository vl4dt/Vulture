<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  let entries = $state<any[]>([]);

  $effect(() => {
    invoke('get_history', { limit: { value: 10 } }).then((history) => {
      entries = history as any[];
    });
  });

  function timeAgo(dateStr: string): string {
    const date = new Date(dateStr);
    const now = new Date();
    const diff = Math.floor((now.getTime() - date.getTime()) / 1000);
    if (diff < 60) return 'just now';
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
    if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
    return `${Math.floor(diff / 86400)}d ago`;
  }
</script>

<div class="fui-glass p-4">
  <h3 class="text-xs font-mono text-fui-white/40 mb-3 tracking-widest">ACTIVITY STREAM</h3>

  <div class="space-y-2 max-h-48 overflow-auto">
    {#if entries.length === 0}
      <div class="text-sm font-mono text-fui-white/30 text-center py-4">No recent activity</div>
    {:else}
      {#each entries.slice(0, 10) as entry (entry.id || $index)}
        <div class="flex items-center gap-3 text-xs font-mono animate-fade-in">
          <!-- Timestamp -->
          <span class="text-fui-white/20 w-16 shrink-0">
            {timeAgo(entry.executed_at || '')}
          </span>

          <!-- Icon -->
          <span class="text-fui-teal">✓</span>

          <!-- Description -->
          <span class="text-fui-white/50 flex-1 truncate">
            Cleanup: {entry.items_processed || 0} items, {entry.space_freed ? (entry.space_freed / 1024 / 1024).toFixed(1) + ' MB' : '0 MB'} freed
          </span>
        </div>
      {/each}
    {/if}
  </div>
</div>
