<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { riskTolerance } from '../stores/app';
  import GlassPanel from '../components/GlassPanel.svelte';
  import NeonButton from '../components/NeonButton.svelte';

  let config = $state<any>(null);
  let loading = $state(false);

  $effect(() => {
    invoke('get_config').then((c) => {
      config = c as any;
    });
  });

  const riskLevels = ['paranoid', 'conservative', 'balanced', 'aggressive', 'nuclear'];
</script>

<div class="space-y-6 animate-fade-in">
  <!-- General Settings -->
  <GlassPanel panelClass="p-6">
    <h3 class="text-xs font-mono text-fui-white/40 mb-4 tracking-widest">GENERAL</h3>

    <div class="space-y-4">
      <div>
        <label class="text-xs font-mono text-fui-white/40 block mb-1">LANGUAGE</label>
        <div class="text-sm font-mono text-fui-white/60">{config?.general?.language || 'en'}</div>
      </div>

      <div>
        <label class="text-xs font-mono text-fui-white/40 block mb-1">OUTPUT FORMAT</label>
        <div class="text-sm font-mono text-fui-white/60">{config?.general?.output_format || 'table'}</div>
      </div>

      <div>
        <label class="text-xs font-mono text-fui-white/40 block mb-1">LOG LEVEL</label>
        <div class="text-sm font-mono text-fui-white/60">{config?.general?.log_level || 'info'}</div>
      </div>
    </div>
  </GlassPanel>

  <!-- Scan Settings -->
  <GlassPanel panelClass="p-6">
    <h3 class="text-xs font-mono text-fui-white/40 mb-4 tracking-widest">SCAN</h3>

    <div class="space-y-4">
      <div>
        <label class="text-xs font-mono text-fui-white/40 block mb-1">DEFAULT DEPTH</label>
        <div class="text-sm font-mono text-fui-white/60">{config?.scan?.default_depth || 'unlimited'}</div>
      </div>

      <label class="flex items-center gap-3 cursor-pointer">
        <input type="checkbox" class="sr-only peer" checked={config?.scan?.scan_hidden} />
        <div class="w-10 h-5 bg-white/10 rounded-full peer-checked:bg-fui-cyan/30 relative transition-colors">
          <div class="w-3 h-3 bg-fui-white/60 rounded-full absolute top-1 left-1 peer-checked:left-6 transition-all"></div>
        </div>
        <span class="text-sm font-mono text-fui-white/60">SCAN HIDDEN FILES</span>
      </label>

      <label class="flex items-center gap-3 cursor-pointer">
        <input type="checkbox" class="sr-only peer" checked={config?.scan?.scan_system} />
        <div class="w-10 h-5 bg-white/10 rounded-full peer-checked:bg-fui-cyan/30 relative transition-colors">
          <div class="w-3 h-3 bg-fui-white/60 rounded-full absolute top-1 left-1 peer-checked:left-6 transition-all"></div>
        </div>
        <span class="text-sm font-mono text-fui-white/60">SCAN SYSTEM DIRECTORIES</span>
      </label>
    </div>
  </GlassPanel>

  <!-- Analysis Settings -->
  <GlassPanel panelClass="p-6">
    <h3 class="text-xs font-mono text-fui-white/40 mb-4 tracking-widest">ANALYSIS</h3>

    <div class="space-y-4">
      <div>
        <label class="text-xs font-mono text-fui-white/40 block mb-2">RISK TOLERANCE</label>
        <div class="flex gap-2">
          {#each riskLevels as level}
            <button
              class={`px-3 py-1 rounded text-xs font-mono transition-all fui-hover ${$riskTolerance === level ? 'bg-fui-cyan/20 text-fui-cyan' : 'text-fui-white/40'}`}
              onclick={() => riskTolerance.set(level)}
            >
              {level.toUpperCase()}
            </button>
          {/each}
        </div>
      </div>

      <div>
        <label class="text-xs font-mono text-fui-white/40 block mb-1">UNUSED APP THRESHOLD</label>
        <div class="text-sm font-mono text-fui-white/60">{config?.analysis?.unused_app_threshold_days || 90} days</div>
      </div>
    </div>
  </GlassPanel>

  <!-- LLM Settings -->
  <GlassPanel panelClass="p-6">
    <h3 class="text-xs font-mono text-fui-white/40 mb-4 tracking-widest">LLM (OPTIONAL)</h3>

    <div class="space-y-4">
      <label class="flex items-center gap-3 cursor-pointer">
        <input type="checkbox" class="sr-only peer" checked={config?.llm?.enabled} />
        <div class="w-10 h-5 bg-white/10 rounded-full peer-checked:bg-fui-purple/30 relative transition-colors">
          <div class="w-3 h-3 bg-fui-white/60 rounded-full absolute top-1 left-1 peer-checked:left-6 transition-all"></div>
        </div>
        <span class="text-sm font-mono text-fui-white/60">ENABLE LLM ANALYSIS</span>
      </label>

      <div>
        <label class="text-xs font-mono text-fui-white/40 block mb-1">PROVIDER</label>
        <div class="text-sm font-mono text-fui-white/60">{config?.llm?.provider || 'openai'}</div>
      </div>

      <div>
        <label class="text-xs font-mono text-fui-white/40 block mb-1">MODEL</label>
        <div class="text-sm font-mono text-fui-white/60">{config?.llm?.model || 'gpt-4o-mini'}</div>
      </div>

      <div>
        <label class="text-xs font-mono text-fui-white/40 block mb-1">MAX CALLS PER SCAN</label>
        <div class="text-sm font-mono text-fui-white/60">{config?.llm?.max_calls_per_scan || 50}</div>
      </div>
    </div>
  </GlassPanel>

  <!-- Cleanup Settings -->
  <GlassPanel panelClass="p-6">
    <h3 class="text-xs font-mono text-fui-white/40 mb-4 tracking-widest">CLEANUP</h3>

    <div class="space-y-4">
      <label class="flex items-center gap-3 cursor-pointer">
        <input type="checkbox" class="sr-only peer" checked={config?.cleanup?.quarantine_enabled} />
        <div class="w-10 h-5 bg-white/10 rounded-full peer-checked:bg-fui-cyan/30 relative transition-colors">
          <div class="w-3 h-3 bg-fui-white/60 rounded-full absolute top-1 left-1 peer-checked:left-6 transition-all"></div>
        </div>
        <span class="text-sm font-mono text-fui-white/60">QUARANTINE ENABLED</span>
      </label>

      <div>
        <label class="text-xs font-mono text-fui-white/40 block mb-1">CONFIRMATION THRESHOLD</label>
        <div class="text-sm font-mono text-fui-white/60">{config?.cleanup?.confirm_threshold_mb || 1000} MB</div>
      </div>
    </div>
  </GlassPanel>
</div>
