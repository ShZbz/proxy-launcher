<script setup lang="ts">
import { computed, ref } from 'vue';
import type { Port, ProbeResult, Rule } from '../types';
import { getLatencyColor, getReasonStyle } from '../types';

const props = defineProps<{
  ports: Port[];
  rules: Rule[];
  probeResults: Record<string, ProbeResult>;
  probingIds: Record<string, boolean>;
  selectedPortId: string | null;
  isDark: boolean;
}>();

const emit = defineEmits<{
  'select-port': [portId: string | null];
  'probe-port': [port: Port];
  'probe-all': [];
  'add-port': [];
  'edit-port': [port: Port];
  'delete-port': [port: Port];
  'open-terminal': [port: Port, shell?: 'powershell' | 'cmd'];
  'open-terminal-snippet': [port: Port];
  'toggle-theme': [];
}>();

const logoFailed = ref(false);

const ruleCounts = computed(() => {
  const counts: Record<string, number> = {};
  for (const r of props.rules) {
    counts[r.port_id] = (counts[r.port_id] ?? 0) + 1;
  }
  return counts;
});

const defaultPort = computed<Port | null>(() => {
  if (props.selectedPortId) {
    const found = props.ports.find((p) => p.id === props.selectedPortId);
    if (found) return found;
  }
  return props.ports[0] ?? null;
});

const anyProbing = computed(() =>
  props.ports.some((p) => Boolean(props.probingIds[p.id])),
);

function handleTopTerminal() {
  if (defaultPort.value) {
    emit('open-terminal', defaultPort.value, 'powershell');
  }
}

function handleTopTerminalSnippet() {
  if (defaultPort.value) {
    emit('open-terminal-snippet', defaultPort.value);
  }
}

function togglePortFilter(portId: string) {
  emit('select-port', props.selectedPortId === portId ? null : portId);
}

function getCardStatus(port: Port) {
  const isProbing = Boolean(props.probingIds[port.id]);
  const res = props.probeResults[port.id];
  const reason = isProbing ? 'TESTING' : (res?.reason || 'UNTESTED');
  const style = getReasonStyle(reason, props.isDark);
  return { isProbing, res, style };
}
</script>

<template>
  <aside
    class="w-[340px] shrink-0 h-full flex flex-col border-r select-none transition-colors duration-150"
    :style="{
      backgroundColor: isDark ? '#0E1116' : '#E7ECF4',
      borderColor: isDark ? '#2B3240' : '#D8E0EC',
      color: isDark ? '#ECF0F8' : '#171B24',
    }"
  >
    <!-- Brand Top Bar -->
    <div class="px-3 pt-3 pb-2 flex items-center gap-2">
      <div
        class="w-8 h-8 rounded-lg overflow-hidden shrink-0 flex items-center justify-center shadow-sm"
        :style="{ backgroundColor: isDark ? '#1B1F28' : '#FFFFFF' }"
      >
        <img
          v-if="!logoFailed"
          src="/app_icon_64.png"
          alt="Proxy Launcher"
          class="w-8 h-8 object-cover"
          @error="logoFailed = true"
        />
        <svg
          v-else
          class="w-5 h-5"
          :style="{ color: isDark ? '#60A5FA' : '#3B82F6' }"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path d="M12 2L2 7l10 5 10-5-10-5z" />
          <path d="M2 17l10 5 10-5" />
          <path d="M2 12l10 5 10-5" />
        </svg>
      </div>

      <span class="font-bold text-[14px] tracking-tight truncate flex-1">
        Proxy Launcher
      </span>

      <button
        type="button"
        :disabled="!defaultPort"
        title="一键打开注入代理端口的终端（右键查看注入代码）"
        class="h-7 px-2.5 rounded-lg text-[11px] font-semibold inline-flex items-center gap-1 transition-colors cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
        :style="{
          backgroundColor: isDark ? '#112E2B' : '#CCFBF1',
          color: isDark ? '#2DD4BF' : '#0D9488',
        }"
        @click="handleTopTerminal"
        @contextmenu.prevent="handleTopTerminalSnippet"
      >
        <span class="font-mono font-bold">&gt;_</span>
        <span>终端</span>
      </button>

      <button
        type="button"
        title="切换深色 / 浅色外观"
        class="h-7 px-2 rounded-lg text-[11px] font-medium inline-flex items-center gap-1 transition-colors cursor-pointer hover:opacity-80"
        :style="{
          backgroundColor: isDark ? '#1B1F28' : '#FFFFFF',
          color: isDark ? '#8B95A9' : '#5C6678',
          border: `1px solid ${isDark ? '#2B3240' : '#D8E0EC'}`,
        }"
        @click="emit('toggle-theme')"
      >
        <svg
          v-if="isDark"
          class="w-3.5 h-3.5"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <circle cx="12" cy="12" r="4" />
          <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41" />
        </svg>
        <svg
          v-else
          class="w-3.5 h-3.5"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
        </svg>
        <span>{{ isDark ? '浅色' : '深色' }}</span>
      </button>
    </div>

    <!-- Ports Section Header -->
    <div class="px-3 py-2 flex items-center gap-1.5">
      <h2
        class="text-[12px] font-bold tracking-wide uppercase flex-1 truncate"
        :style="{ color: isDark ? '#8B95A9' : '#5C6678' }"
      >
        Ports · 代理端口
      </h2>

      <button
        type="button"
        title="显示全部代理端口的规则"
        class="h-6 px-2 rounded-md text-[11px] font-semibold transition-colors cursor-pointer"
        :style="
          selectedPortId === null
            ? {
                backgroundColor: isDark ? '#1A2B4C' : '#DBEAFE',
                color: isDark ? '#93C5FD' : '#1D4ED8',
              }
            : {
                backgroundColor: isDark ? '#242A36' : '#DCE4F0',
                color: isDark ? '#8B95A9' : '#5C6678',
              }
        "
        @click="emit('select-port', null)"
      >
        全部
      </button>

      <button
        type="button"
        :disabled="ports.length === 0 || anyProbing"
        title="测试所有端口连通性 (Ctrl+T)"
        class="h-6 px-2 rounded-md text-[11px] font-semibold transition-colors cursor-pointer disabled:opacity-45 disabled:cursor-not-allowed"
        :style="{
          backgroundColor: isDark ? '#242A36' : '#FFFFFF',
          color: isDark ? '#60A5FA' : '#2563EB',
          border: `1px solid ${isDark ? '#2B3240' : '#D8E0EC'}`,
        }"
        @click="emit('probe-all')"
      >
        {{ anyProbing ? '测试中…' : '全部测试' }}
      </button>

      <button
        type="button"
        title="新增代理端口"
        class="h-6 px-2 rounded-md text-[11px] font-bold text-white inline-flex items-center gap-0.5 transition-colors cursor-pointer"
        :style="{ backgroundColor: isDark ? '#60A5FA' : '#3B82F6' }"
        @click="emit('add-port')"
      >
        <span>+</span>
        <span>新增</span>
      </button>
    </div>

    <!-- Scrollable Port Cards List -->
    <div class="flex-1 overflow-y-auto px-2.5 pb-2 space-y-2">
      <div
        v-if="ports.length === 0"
        class="mt-10 px-4 py-6 rounded-xl text-center border border-dashed"
        :style="{
          borderColor: isDark ? '#2B3240' : '#C9D4E4',
          color: isDark ? '#8B95A9' : '#5C6678',
        }"
      >
        <p class="text-sm font-semibold mb-1">还没有代理端口</p>
        <p class="text-xs mb-3 opacity-80">点击右上角「+ 新增」添加本地代理端口</p>
        <button
          type="button"
          class="px-3 py-1.5 rounded-lg text-xs font-semibold text-white cursor-pointer"
          :style="{ backgroundColor: isDark ? '#60A5FA' : '#3B82F6' }"
          @click="emit('add-port')"
        >
          + 添加端口
        </button>
      </div>

      <div
        v-for="port in ports"
        :key="port.id"
        class="port-card min-h-[136px] rounded-[10px] p-3 flex flex-col justify-between cursor-pointer transition-all duration-150"
        :style="{
          backgroundColor:
            selectedPortId === port.id
              ? isDark
                ? '#16243A'
                : '#E8F1FF'
              : isDark
                ? '#1B1F28'
                : '#FFFFFF',
          border:
            selectedPortId === port.id
              ? `2px solid ${isDark ? '#60A5FA' : '#2563EB'}`
              : `1px solid ${isDark ? '#252B38' : '#E2E8F2'}`,
        }"
        @click="togglePortFilter(port.id)"
      >
        <!-- Row 0: Dot + Alias + Rule Count Badge + >_ Button -->
        <div class="flex items-center gap-2 min-w-0">
          <span
            class="w-2.5 h-2.5 rounded-full shrink-0 transition-colors"
            :class="{ 'animate-pulse-dot': getCardStatus(port).isProbing }"
            :style="{ backgroundColor: getCardStatus(port).style.fg }"
          />

          <span class="font-bold text-[14px] truncate flex-1">
            {{ port.alias || '未命名' }}
          </span>

          <span
            class="px-1.5 py-0.5 rounded text-[10px] font-medium shrink-0"
            :style="{
              backgroundColor: isDark ? '#242A36' : '#EBF0F7',
              color: isDark ? '#8B95A9' : '#5C6678',
            }"
          >
            {{ ruleCounts[port.id] ?? 0 }} 条规则
          </span>

          <button
            type="button"
            title="打开注入该端口的 PowerShell 终端（右键查看注入代码）"
            class="w-7 h-6 rounded-md font-mono text-[11px] font-bold flex items-center justify-center shrink-0 transition-colors cursor-pointer"
            :style="{
              backgroundColor: isDark ? '#112E2B' : '#CCFBF1',
              color: isDark ? '#2DD4BF' : '#0D9488',
            }"
            @click.stop="emit('open-terminal', port, 'powershell')"
            @contextmenu.prevent.stop="emit('open-terminal-snippet', port)"
          >
            &gt;_
          </button>
        </div>

        <!-- Row 1: Host:Port Monospace -->
        <div
          class="font-mono text-[11px] mt-0.5 truncate pl-[18px]"
          :style="{ color: isDark ? '#8B95A9' : '#5C6678' }"
        >
          {{ port.host }}:{{ port.port }}
        </div>

        <!-- Row 2: Latency + Exit IP / Error Detail -->
        <div class="flex items-center justify-between gap-2 mt-2 pl-[18px]">
          <span
            class="text-[13px] font-bold tabular-nums"
            :style="{
              color: getCardStatus(port).res?.ok
                ? getLatencyColor(getCardStatus(port).res?.latency_ms ?? null, isDark)
                : isDark
                  ? '#8B95A9'
                  : '#5C6678',
            }"
          >
            {{
              getCardStatus(port).res?.ok &&
              getCardStatus(port).res?.latency_ms != null
                ? `${getCardStatus(port).res?.latency_ms} ms`
                : '—'
            }}
          </span>

          <span
            v-if="getCardStatus(port).res?.ok && getCardStatus(port).res?.exit_ip"
            class="font-mono text-[11px] truncate"
            :style="{ color: isDark ? '#8B95A9' : '#5C6678' }"
            :title="`出口 IP: ${getCardStatus(port).res?.exit_ip}`"
          >
            {{ getCardStatus(port).res?.exit_ip }}
          </span>
          <span
            v-else-if="getCardStatus(port).res && !getCardStatus(port).res?.ok && getCardStatus(port).res?.detail"
            class="text-[11px] truncate max-w-[165px]"
            :style="{ color: isDark ? '#8B95A9' : '#5C6678' }"
            :title="`${getCardStatus(port).res?.reason_text}\n${getCardStatus(port).res?.detail}`"
          >
            {{ getCardStatus(port).res?.detail }}
          </span>
        </div>

        <!-- Row 3: Status Pill + Card Actions -->
        <div class="flex items-center justify-between gap-2 mt-2.5">
          <span
            class="px-2.5 py-0.5 rounded-full text-[11px] font-semibold leading-tight"
            :style="{
              color: getCardStatus(port).style.fg,
              backgroundColor: getCardStatus(port).style.bg,
            }"
          >
            {{ getCardStatus(port).style.label }}
          </span>

          <div class="flex items-center gap-1" @click.stop>
            <button
              type="button"
              :disabled="getCardStatus(port).isProbing"
              class="h-6 px-2.5 rounded-full text-[11px] font-semibold transition-colors cursor-pointer disabled:opacity-60"
              :style="{
                backgroundColor: isDark ? '#242A36' : '#EBF0F7',
                color: isDark ? '#60A5FA' : '#3B82F6',
              }"
              @click.stop="emit('probe-port', port)"
            >
              {{ getCardStatus(port).isProbing ? '…' : '测试' }}
            </button>

            <button
              type="button"
              title="编辑端口"
              class="h-6 px-2 rounded-md text-[11px] font-medium transition-colors cursor-pointer"
              :style="{
                backgroundColor: isDark ? '#242A36' : '#F3F5F9',
                color: isDark ? '#8B95A9' : '#5C6678',
              }"
              @click.stop="emit('edit-port', port)"
            >
              编辑
            </button>

            <button
              type="button"
              title="删除端口"
              class="h-6 px-2 rounded-md text-[11px] font-medium transition-colors cursor-pointer"
              :style="{
                backgroundColor: isDark ? '#3B181B' : '#FEE2E2',
                color: isDark ? '#F87171' : '#DC2626',
              }"
              @click.stop="emit('delete-port', port)"
            >
              删除
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Bottom Bar -->
    <div
      class="px-3 py-2.5 border-t flex items-center justify-between gap-2"
      :style="{ borderColor: isDark ? '#2B3240' : '#D8E0EC' }"
    >
      <span
        class="text-[11px] truncate"
        :style="{ color: isDark ? '#8B95A9' : '#5C6678' }"
      >
        单击卡片筛选 · 右键 &gt;_ 看代码
      </span>

      <button
        type="button"
        :disabled="!defaultPort"
        title="查看或复制终端代理环境变量注入代码"
        class="h-7 px-2.5 rounded-lg text-[11px] font-semibold shrink-0 transition-colors cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
        :style="{
          backgroundColor: isDark ? '#1B1F28' : '#FFFFFF',
          color: isDark ? '#ECF0F8' : '#171B24',
          border: `1px solid ${isDark ? '#2B3240' : '#D8E0EC'}`,
        }"
        @click="handleTopTerminalSnippet"
      >
        注入命令
      </button>
    </div>
  </aside>
</template>

<style scoped>
.port-card:hover {
  transform: translateY(-1px);
}

@keyframes pulse-dot {
  0%,
  100% {
    opacity: 1;
    transform: scale(1);
  }
  50% {
    opacity: 0.35;
    transform: scale(0.8);
  }
}

.animate-pulse-dot {
  animation: pulse-dot 0.9s ease-in-out infinite;
}
</style>
