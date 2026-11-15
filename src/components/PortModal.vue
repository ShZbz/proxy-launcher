<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from 'vue';
import type { Port } from '../types';

const props = defineProps<{
  visible: boolean;
  editingPort: Port | null;
  isDark: boolean;
}>();

const emit = defineEmits<{
  close: [];
  save: [portData: { id?: string; alias: string; host: string; port: number }];
}>();

const alias = ref('');
const host = ref('127.0.0.1');
const portStr = ref('');
const errorMsg = ref('');
const aliasInputRef = ref<HTMLInputElement | null>(null);

watch(
  () => [props.visible, props.editingPort] as const,
  async ([isVisible]) => {
    if (!isVisible) return;
    errorMsg.value = '';
    if (props.editingPort) {
      alias.value = props.editingPort.alias;
      host.value = props.editingPort.host;
      portStr.value = String(props.editingPort.port);
    } else {
      alias.value = '';
      host.value = '127.0.0.1';
      portStr.value = '';
    }
    await nextTick();
    aliasInputRef.value?.focus();
  },
  { immediate: true },
);

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
});

function handleSubmit() {
  errorMsg.value = '';
  const cleanHost = host.value.trim() || '127.0.0.1';
  const rawPort = portStr.value.trim();

  if (!rawPort || !/^\d+$/.test(rawPort)) {
    errorMsg.value = '端口必须是 1 到 65535 之间的整数';
    return;
  }

  const portNum = Number.parseInt(rawPort, 10);
  if (Number.isNaN(portNum) || portNum < 1 || portNum > 65535) {
    errorMsg.value = '端口必须在 1 到 65535 之间';
    return;
  }

  if (/\s/.test(cleanHost)) {
    errorMsg.value = '主机地址不能包含空白字符';
    return;
  }

  const payload: { id?: string; alias: string; host: string; port: number } = {
    alias: alias.value.trim(),
    host: cleanHost,
    port: portNum,
  };
  if (props.editingPort) {
    payload.id = props.editingPort.id;
  }

  emit('save', payload);
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="visible"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-[2px] p-4"
      @mousedown.self="emit('close')"
    >
      <div
        class="w-full max-w-[400px] rounded-xl shadow-2xl border p-5 transition-colors"
        :style="{
          backgroundColor: isDark ? '#12151B' : '#F3F5F9',
          borderColor: isDark ? '#2B3240' : '#D8E0EC',
          color: isDark ? '#ECF0F8' : '#171B24',
        }"
      >
        <div class="flex items-center justify-between mb-4">
          <h3 class="text-[16px] font-bold">
            {{ editingPort ? '编辑端口' : '新建端口' }}
          </h3>
          <button
            type="button"
            class="w-7 h-7 rounded-lg flex items-center justify-center text-sm cursor-pointer transition-colors"
            :style="{ color: isDark ? '#8B95A9' : '#5C6678' }"
            @click="emit('close')"
          >
            ✕
          </button>
        </div>

        <form class="space-y-3" @submit.prevent="handleSubmit">
          <div>
            <label
              class="block text-[11px] font-medium mb-1"
              :style="{ color: isDark ? '#8B95A9' : '#5C6678' }"
            >
              别名
            </label>
            <input
              ref="aliasInputRef"
              v-model="alias"
              type="text"
              placeholder="例如 clash / Verge mixed"
              class="w-full h-9 px-3 rounded-lg text-[13px] outline-none border transition-colors"
              :style="{
                backgroundColor: isDark ? '#1B1F28' : '#FFFFFF',
                borderColor: isDark ? '#2B3240' : '#D8E0EC',
                color: isDark ? '#ECF0F8' : '#171B24',
              }"
            />
          </div>

          <div>
            <label
              class="block text-[11px] font-medium mb-1"
              :style="{ color: isDark ? '#8B95A9' : '#5C6678' }"
            >
              主机
            </label>
            <input
              v-model="host"
              type="text"
              placeholder="127.0.0.1"
              class="w-full h-9 px-3 rounded-lg text-[13px] font-mono outline-none border transition-colors"
              :style="{
                backgroundColor: isDark ? '#1B1F28' : '#FFFFFF',
                borderColor: isDark ? '#2B3240' : '#D8E0EC',
                color: isDark ? '#ECF0F8' : '#171B24',
              }"
            />
          </div>

          <div>
            <label
              class="block text-[11px] font-medium mb-1"
              :style="{ color: isDark ? '#8B95A9' : '#5C6678' }"
            >
              端口 (1–65535)
            </label>
            <input
              v-model="portStr"
              type="text"
              inputmode="numeric"
              placeholder="7890"
              class="w-full h-9 px-3 rounded-lg text-[13px] font-mono outline-none border transition-colors"
              :style="{
                backgroundColor: isDark ? '#1B1F28' : '#FFFFFF',
                borderColor: isDark ? '#2B3240' : '#D8E0EC',
                color: isDark ? '#ECF0F8' : '#171B24',
              }"
            />
          </div>

          <p
            v-if="errorMsg"
            class="text-[12px] font-medium pt-0.5"
            :style="{ color: isDark ? '#F87171' : '#DC2626' }"
          >
            {{ errorMsg }}
          </p>

          <div class="flex items-center justify-end gap-2 pt-3">
            <button
              type="button"
              class="h-8 px-4 rounded-lg text-[13px] font-medium border cursor-pointer transition-colors"
              :style="{
                borderColor: isDark ? '#2B3240' : '#D8E0EC',
                color: isDark ? '#8B95A9' : '#5C6678',
              }"
              @click="emit('close')"
            >
              取消
            </button>
            <button
              type="submit"
              class="h-8 px-5 rounded-lg text-[13px] font-semibold text-white cursor-pointer transition-colors"
              :style="{ backgroundColor: isDark ? '#60A5FA' : '#3B82F6' }"
            >
              保存
            </button>
          </div>
        </form>
      </div>
    </div>
  </Teleport>
</template>
