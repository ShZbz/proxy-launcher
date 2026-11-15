<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import type { Port } from '../types';

type ShellTab = 'powershell' | 'cmd' | 'bash';

const props = defineProps<{
  visible: boolean;
  port: Port | null;
  isDark: boolean;
}>();

const emit = defineEmits<{
  close: [];
  'launch-shell': [portId: string, shell: 'powershell' | 'cmd'];
  notify: [msg: string];
}>();

const activeTab = ref<ShellTab>('powershell');
const copied = ref(false);
let copyTimer: ReturnType<typeof setTimeout> | null = null;

const NO_PROXY_VALUE = 'localhost,127.0.0.1,::1';
const AICODE_ENDPOINT_URL = 'https://localhost@aicode.googleapis.com';

const targetHost = computed(() => props.port?.host || '127.0.0.1');
const targetPort = computed(() => props.port?.port ?? 7890);
const httpUrl = computed(() => `http://${targetHost.value}:${targetPort.value}`);
const socksUrl = computed(() => `socks5://${targetHost.value}:${targetPort.value}`);

const snippets = computed<Record<ShellTab, string>>(() => {
  const h = httpUrl.value;
  const s = socksUrl.value;
  return {
    powershell:
      `$env:HTTP_PROXY="${h}"; ` +
      `$env:HTTPS_PROXY="${h}"; ` +
      `$env:ALL_PROXY="${s}"; ` +
      `$env:NO_PROXY="${NO_PROXY_VALUE}"; ` +
      `$env:AICODE_ENDPOINT_URL="${AICODE_ENDPOINT_URL}"`,
    cmd:
      `set "HTTP_PROXY=${h}" & ` +
      `set "HTTPS_PROXY=${h}" & ` +
      `set "ALL_PROXY=${s}" & ` +
      `set "NO_PROXY=${NO_PROXY_VALUE}" & ` +
      `set "AICODE_ENDPOINT_URL=${AICODE_ENDPOINT_URL}"`,
    bash:
      `export HTTP_PROXY="${h}" ` +
      `HTTPS_PROXY="${h}" ` +
      `ALL_PROXY="${s}" ` +
      `NO_PROXY="${NO_PROXY_VALUE}" ` +
      `AICODE_ENDPOINT_URL="${AICODE_ENDPOINT_URL}"`,
  };
});

const tabLabels: { id: ShellTab; label: string; subtitle: string }[] = [
  { id: 'powershell', label: 'PowerShell', subtitle: 'pwsh / Windows PowerShell' },
  { id: 'cmd', label: 'CMD', subtitle: 'Windows 命令提示符' },
  { id: 'bash', label: 'Bash', subtitle: 'Git Bash / WSL / Linux' },
];

const currentSnippet = computed(() => snippets.value[activeTab.value]);

async function copySnippet() {
  const text = currentSnippet.value;
  try {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      await navigator.clipboard.writeText(text);
    } else {
      const ta = document.createElement('textarea');
      ta.value = text;
      ta.style.position = 'fixed';
      ta.style.opacity = '0';
      document.body.appendChild(ta);
      ta.select();
      document.execCommand('copy');
      document.body.removeChild(ta);
    }
    copied.value = true;
    const label = tabLabels.find((t) => t.id === activeTab.value)?.label ?? activeTab.value;
    emit('notify', `已复制 ${label} 代理环境变量注入命令`);
    if (copyTimer) clearTimeout(copyTimer);
    copyTimer = setTimeout(() => {
      copied.value = false;
    }, 1500);
  } catch (err) {
    emit('notify', `复制失败: ${String(err)}`);
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (!props.visible) return;
  if (e.key === 'Escape') {
    e.preventDefault();
    emit('close');
  }
}

watch(
  () => props.visible,
  (isVisible) => {
    copied.value = false;
    if (isVisible) {
      window.addEventListener('keydown', handleKeydown);
    } else {
      window.removeEventListener('keydown', handleKeydown);
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeydown);
  if (copyTimer) clearTimeout(copyTimer);
});
</script>

<template>
  <Teleport to="body">
    <div
      v-if="visible"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-[2px] p-4"
      @mousedown.self="emit('close')"
    >
      <div
        class="w-full max-w-[600px] rounded-xl shadow-2xl border p-5 transition-colors"
        :style="{
          backgroundColor: isDark ? '#12151B' : '#F3F5F9',
          borderColor: isDark ? '#2B3240' : '#D8E0EC',
          color: isDark ? '#ECF0F8' : '#171B24',
        }"
      >
        <!-- Header -->
        <div class="flex items-center justify-between gap-3 mb-4">
          <div class="min-w-0">
            <h3 class="text-[16px] font-bold">代理终端与环境变量注入代码</h3>
            <p
              class="text-[12px] mt-0.5 truncate"
              :style="{ color: isDark ? '#8B95A9' : '#5C6678' }"
            >
              目标端口：
              <span class="font-semibold" :style="{ color: isDark ? '#ECF0F8' : '#171B24' }">
                {{ port ? `${port.alias || '端口'} (${port.host}:${port.port})` : '127.0.0.1:7890 (未选择)' }}
              </span>
            </p>
          </div>
          <button
            type="button"
            class="w-7 h-7 rounded-lg flex items-center justify-center text-sm cursor-pointer shrink-0"
            :style="{ color: isDark ? '#8B95A9' : '#5C6678' }"
            @click="emit('close')"
          >
            ✕
          </button>
        </div>

        <!-- Direct Terminal Launch Buttons -->
        <div class="flex flex-wrap items-center gap-2 mb-4">
          <button
            type="button"
            :disabled="!port"
            class="h-8 px-3.5 rounded-lg text-[12px] font-semibold text-white inline-flex items-center gap-1.5 cursor-pointer transition-colors disabled:opacity-45 disabled:cursor-not-allowed"
            :style="{ backgroundColor: isDark ? '#0D9488' : '#0D9488' }"
            @click="port && emit('launch-shell', port.id, 'powershell')"
          >
            <span class="font-mono font-bold">&gt;_</span>
            <span>启动 PowerShell 终端</span>
          </button>

          <button
            type="button"
            :disabled="!port"
            class="h-8 px-3.5 rounded-lg text-[12px] font-semibold inline-flex items-center gap-1.5 border cursor-pointer transition-colors disabled:opacity-45 disabled:cursor-not-allowed"
            :style="{
              backgroundColor: isDark ? '#242A36' : '#FFFFFF',
              borderColor: isDark ? '#2B3240' : '#D8E0EC',
              color: isDark ? '#ECF0F8' : '#171B24',
            }"
            @click="port && emit('launch-shell', port.id, 'cmd')"
          >
            <span class="font-mono font-bold">&gt;_</span>
            <span>启动 CMD 终端</span>
          </button>
        </div>

        <!-- Instruction -->
        <p
          class="text-[12px] mb-2.5"
          :style="{ color: isDark ? '#8B95A9' : '#5C6678' }"
        >
          若需要在已打开的终端或管理员终端中手动注入代理，可切换 Shell 类型并一键复制命令：
        </p>

        <!-- Tab Switcher -->
        <div
          class="inline-flex p-1 rounded-lg border mb-3"
          :style="{
            backgroundColor: isDark ? '#1B1F28' : '#E7ECF4',
            borderColor: isDark ? '#2B3240' : '#D8E0EC',
          }"
        >
          <button
            v-for="tab in tabLabels"
            :key="tab.id"
            type="button"
            class="px-3 py-1 rounded-md text-[12px] font-semibold transition-colors cursor-pointer"
            :style="
              activeTab === tab.id
                ? {
                    backgroundColor: isDark ? '#242A36' : '#FFFFFF',
                    color: isDark ? '#60A5FA' : '#2563EB',
                  }
                : {
                    color: isDark ? '#8B95A9' : '#5C6678',
                  }
            "
            @click="activeTab = tab.id; copied = false"
          >
            {{ tab.label }}
          </button>
        </div>

        <!-- Code Surface -->
        <div
          class="rounded-lg border p-3"
          :style="{
            backgroundColor: isDark ? '#0B0E14' : '#1E2430',
            borderColor: isDark ? '#232A38' : '#333D4F',
          }"
        >
          <div class="flex items-center justify-between gap-2 mb-2">
            <span class="text-[11px] font-medium text-[#94A3B8]">
              {{ tabLabels.find((t) => t.id === activeTab)?.subtitle }}
            </span>

            <button
              type="button"
              class="h-7 px-3 rounded-md text-[11px] font-semibold text-white transition-colors cursor-pointer"
              :style="{
                backgroundColor: copied
                  ? '#0D9488'
                  : isDark
                    ? '#3B82F6'
                    : '#2563EB',
              }"
              @click="copySnippet"
            >
              {{ copied ? '✓ 已复制' : '复制命令' }}
            </button>
          </div>

          <pre
            class="font-mono text-[12px] leading-relaxed text-[#E2E8F0] whitespace-pre-wrap break-all select-all m-0"
          >{{ currentSnippet }}</pre>
        </div>

        <!-- Env Vars Summary -->
        <div
          class="mt-3 px-3 py-2 rounded-lg text-[11px] font-mono grid grid-cols-1 gap-0.5"
          :style="{
            backgroundColor: isDark ? '#1B1F28' : '#EBF0F7',
            color: isDark ? '#8B95A9' : '#5C6678',
          }"
        >
          <div>HTTP_PROXY / HTTPS_PROXY = {{ httpUrl }}</div>
          <div>ALL_PROXY = {{ socksUrl }}</div>
          <div>NO_PROXY = {{ NO_PROXY_VALUE }}</div>
          <div>AICODE_ENDPOINT_URL = {{ AICODE_ENDPOINT_URL }}</div>
        </div>

        <!-- Footer -->
        <div class="flex justify-end mt-4">
          <button
            type="button"
            class="h-8 px-4 rounded-lg text-[13px] font-medium border cursor-pointer transition-colors"
            :style="{
              borderColor: isDark ? '#2B3240' : '#D8E0EC',
              color: isDark ? '#8B95A9' : '#5C6678',
            }"
            @click="emit('close')"
          >
            关闭
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
