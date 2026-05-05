<script lang="ts">
  export let color: 'cyan' | 'teal' | 'amber' | 'crimson' | 'purple' = 'cyan';
  export let disabled: boolean = false;

  const colorMap = {
    cyan: { bg: 'bg-fui-cyan/10', text: 'text-fui-cyan', border: 'border-fui-cyan/30', shadow: 'shadow-glow-cyan', hoverBg: 'hover:bg-fui-cyan/20' },
    teal: { bg: 'bg-fui-teal/10', text: 'text-fui-teal', border: 'border-fui-teal/30', shadow: 'shadow-glow-teal', hoverBg: 'hover:bg-fui-teal/20' },
    amber: { bg: 'bg-fui-amber/10', text: 'text-fui-amber', border: 'border-fui-amber/30', shadow: 'shadow-glow-amber', hoverBg: 'hover:bg-fui-amber/20' },
    crimson: { bg: 'bg-fui-crimson/10', text: 'text-fui-crimson', border: 'border-fui-crimson/30', shadow: 'shadow-glow-crimson', hoverBg: 'hover:bg-fui-crimson/20' },
    purple: { bg: 'bg-fui-purple/10', text: 'text-fui-purple', border: 'border-fui-purple/30', shadow: 'shadow-glow-purple', hoverBg: 'hover:bg-fui-purple/20' },
  };

  const c = colorMap[color];
</script>

<button
  class="px-6 py-3 rounded-lg font-mono text-sm font-bold tracking-wider transition-all duration-200 {c.bg} {c.text} border {c.border} {c.shadow} {c.hoverBg} disabled:opacity-30 disabled:cursor-not-allowed"
  class:animate-pulse-glow={color === 'cyan' && !disabled}
  {disabled}
>
  <slot />
</button>
