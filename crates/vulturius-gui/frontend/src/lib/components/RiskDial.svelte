<script lang="ts">
  import { riskTolerance } from '../stores/app';

  const levels = [
    { id: 'paranoid', label: 'PARANOID', color: 'text-fui-crimson', bg: 'bg-fui-crimson' },
    { id: 'conservative', label: 'CONSERVATIVE', color: 'text-fui-amber', bg: 'bg-fui-amber' },
    { id: 'balanced', label: 'BALANCED', color: 'text-fui-teal', bg: 'bg-fui-teal' },
    { id: 'aggressive', label: 'AGGRESSIVE', color: 'text-fui-cyan', bg: 'bg-fui-cyan' },
    { id: 'nuclear', label: 'NUCLEAR', color: 'text-fui-purple', bg: 'bg-fui-purple' },
  ];

  function selectLevel(id: string) {
    riskTolerance.set(id);
  }
</script>

<div class="relative w-40 h-40">
  <!-- Dial background -->
  <div class="absolute inset-0 rounded-full border border-fui-white/10"></div>

  <!-- Level indicators around the dial -->
  {#each levels as level, i}
    {@const angle = (i * 72) - 90}
    <button
      class={`absolute w-6 h-6 rounded-full border border-fui-white/20 flex items-center justify-center text-xs font-mono transition-all hover:scale-125 cursor-pointer ${level.color} ${$riskTolerance === level.id ? 'bg-fui-cyan/20' : ''}`}
      style="transform: translate({Math.cos(angle * Math.PI / 180) * 35}%, {Math.sin(angle * Math.PI / 180) * 35}%) translate(-50%, -50%); top: 50%; left: 50%;"
      onclick={() => selectLevel(level.id)}
      title={level.label}
    >
      {i + 1}
    </button>
  {/each}

  <!-- Center label -->
  <div class="absolute inset-0 flex items-center justify-center">
    <div class="text-center">
      <div class="text-xs font-mono text-fui-white/40">{levels.find(l => l.id === $riskTolerance)?.label || 'BALANCED'}</div>
    </div>
  </div>
</div>
