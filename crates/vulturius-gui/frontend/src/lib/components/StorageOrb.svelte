<script lang="ts">
  import { systemInfo } from '../stores/app';

  let hovered = $state(false);

  function formatBytes(bytes: number): string {
    if (!bytes) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
  }

  // Calculate usage percentage from drive info
  let driveStats = $derived({
    total: $systemInfo.drives.reduce((sum, d) => sum + (d.total_space || 0), 0),
    used: $systemInfo.drives.reduce((sum, d) => sum + ((d.total_space || 0) - (d.available_space || 0)), 0),
  });
</script>

<div class="relative w-64 h-64 flex items-center justify-center" class:animate-float={hovered}>
  <!-- Outer glow ring -->
  <div class="absolute inset-4 rounded-full border border-fui-cyan/10 animate-spin-slow"></div>
  <div class="absolute inset-8 rounded-full border border-fui-cyan/5 animate-spin-slow" style="animation-direction: reverse;"></div>

  <!-- Orb -->
  <div
    class="relative w-48 h-48 rounded-full cursor-pointer transition-all duration-500"
    style="background: radial-gradient(circle at 35% 35%, rgba(0, 240, 255, 0.15), rgba(0, 212, 170, 0.08), rgba(0, 0, 8, 0.9)); box-shadow: 0 0 60px rgba(0, 240, 255, 0.15), inset 0 0 60px rgba(0, 240, 255, 0.05);"
    class:shadow-glow-cyan={hovered}
    onmouseenter={() => hovered = true}
    onmouseleave={() => hovered = false}
  >
    <!-- Inner data readout -->
    <div class="absolute inset-0 flex flex-col items-center justify-center">
      <div class="text-3xl font-mono data-number text-fui-cyan glow-text-cyan">
        {formatBytes($systemInfo.drives[0]?.total_space || 0)}
      </div>
      <div class="text-xs font-mono text-fui-white/40 mt-1">TOTAL STORAGE</div>
      <div class="text-sm font-mono data-number text-fui-teal mt-2">
        {formatBytes($systemInfo.drives[0]?.available_space || 0)}
      </div>
      <div class="text-xs font-mono text-fui-white/30">AVAILABLE</div>
    </div>

    <!-- Scan line effect -->
    <div class="absolute inset-0 rounded-full overflow-hidden">
      <div class="absolute w-full h-px bg-fui-cyan/10 animate-scan-line"></div>
    </div>
  </div>

  <!-- Orbiting data points -->
  <div class="absolute inset-0 animate-spin-slow">
    <div class="absolute top-2 left-1/2 w-1.5 h-1.5 rounded-full bg-fui-cyan/60"></div>
  </div>
  <div class="absolute inset-0" style="animation: spin 12s linear infinite reverse;">
    <div class="absolute bottom-4 right-8 w-1 h-1 rounded-full bg-fui-teal/60"></div>
  </div>
</div>
