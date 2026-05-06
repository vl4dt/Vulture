<script lang="ts">
  import { currentPage } from '../stores/app';

  interface NavItem {
    id: string;
    label: string;
    icon: string;
  }

  const navItems: NavItem[] = [
    { id: 'dashboard', label: 'DASHBOARD', icon: '◉' },
    { id: 'scanner', label: 'SCANNER', icon: '◈' },
    { id: 'analyzer', label: 'ANALYZER', icon: '⚡' },
    { id: 'cleaner', label: 'CLEANER', icon: '✕' },
    { id: 'apps', label: 'APPS', icon: '▦' },
    { id: 'settings', label: 'SETTINGS', icon: '⚙' },
  ];

  function navigate(id: string) {
    currentPage.set(id);
  }
</script>

<nav class="fixed left-6 top-1/2 -translate-y-1/2 z-50">
  <div class="fui-glass rounded-full p-2 flex flex-col gap-1">
    {#each navItems as item (item.id)}
      <button
        class={`relative w-10 h-10 rounded-full flex items-center justify-center transition-all duration-200 fui-hover ${$currentPage === item.id ? 'bg-white/10 text-fui-cyan shadow-glow-cyan' : ''}`}
        onclick={() => navigate(item.id)}
        title={item.label}
      >
        <span class="text-lg">{item.icon}</span>
        <!-- Active indicator -->
        {#if $currentPage === item.id}
          <span class="absolute -right-1 w-1 h-1 rounded-full bg-fui-cyan animate-pulse"></span>
        {/if}
      </button>
    {/each}
  </div>
</nav>
