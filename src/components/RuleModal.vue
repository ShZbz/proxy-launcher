<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, reactive, ref, watch } from 'vue'
import { api } from '../api'
import type { InjectionMode, Port, ResolvedTarget, Rule } from '../types'
import { generateId, isAntigravityRule } from '../types'

const props = defineProps<{
  visible: boolean
  editingRule: Rule | null
  prefillTarget: ResolvedTarget | null
  ports: Port[]
  defaultPortId: string
  antigravityCdp: boolean
  isDark: boolean
}>()

const emit = defineEmits<{
  close: []
  save: [rule: Rule, createShortcutAfter: boolean]
}>()

const AUTO_BOTH_HINTS = [
  'antigravity',
  'code',
  'chrome',
  'msedge',
  'electron',
  'brave',
  'vivaldi',
]

const MODE_OPTIONS: Array<{
  value: InjectionMode
  title: string
  desc: string
}> = [
  { value: 'env', title: 'env · 环境变量', desc: 'Go / Rust / curl / git' },
  { value: 'flag', title: 'flag · 命令行', desc: 'Chromium 渲染层参数' },
  { value: 'both', title: 'both · 推荐', desc: '环境变量 + 命令行' },
]

const nameInputRef = ref<HTMLInputElement | null>(null)
const errorMsg = ref('')
const needsElevation = ref(false)
const parseNote = ref('')
const browsing = ref(false)

const form = reactive({
  id: '',
  name: '',
  exe: '',
  port_id: '',
  args: '',
  cwd: '',
  mode: 'both' as InjectionMode,
  flag_template: '--proxy-server={url}',
  enabled: true,
  source: '',
})

const isEditMode = computed(() => props.editingRule !== null)

function extractFileStem(pathStr: string): string {
  const normalized = pathStr.trim().replace(/\//g, '\\')
  const base = normalized.split('\\').pop() || ''
  const dotIdx = base.lastIndexOf('.')
  return dotIdx > 0 ? base.slice(0, dotIdx) : base
}

function guessMode(exe: string): InjectionMode {
  const low = (exe || '').toLowerCase()
  return AUTO_BOTH_HINTS.some((hint) => low.includes(hint)) ? 'both' : 'env'
}

function resolveDefaultPortId(): string {
  if (props.defaultPortId && props.ports.some((p) => p.id === props.defaultPortId)) {
    return props.defaultPortId
  }
  return props.ports[0]?.id ?? ''
}

function syncFormFromProps() {
  errorMsg.value = ''
  if (props.editingRule) {
    const r = props.editingRule
    form.id = r.id
    form.name = r.name
    form.exe = r.exe
    form.port_id =
      r.port_id && props.ports.some((p) => p.id === r.port_id)
        ? r.port_id
        : resolveDefaultPortId()
    form.args = r.args
    form.cwd = r.cwd
    form.mode = r.mode || 'both'
    form.flag_template = r.flag_template || '--proxy-server={url}'
    form.enabled = r.enabled
    form.source = r.source || ''
    needsElevation.value = false
    parseNote.value = '已保存的规则'
  } else if (props.prefillTarget) {
    const t = props.prefillTarget
    form.id = generateId()
    form.name = t.name || extractFileStem(t.exe)
    form.exe = t.exe
    form.port_id = resolveDefaultPortId()
    form.args = t.args || ''
    form.cwd = t.cwd || ''
    form.mode = guessMode(t.exe)
    form.flag_template = '--proxy-server={url}'
    form.enabled = true
    form.source = t.source || ''
    needsElevation.value = Boolean(t.needs_elevation)
    parseNote.value = t.parse_note || ''
  } else {
    form.id = generateId()
    form.name = ''
    form.exe = ''
    form.port_id = resolveDefaultPortId()
    form.args = ''
    form.cwd = ''
    form.mode = 'both'
    form.flag_template = '--proxy-server={url}'
    form.enabled = true
    form.source = ''
    needsElevation.value = false
    parseNote.value = ''
  }
}

watch(
  () => [props.visible, props.editingRule, props.prefillTarget] as const,
  ([vis]) => {
    if (vis) {
      syncFormFromProps()
      nextTick(() => {
        nameInputRef.value?.focus()
      })
    }
  },
  { immediate: true },
)

async function handleBrowse() {
  if (browsing.value) return
  browsing.value = true
  errorMsg.value = ''
  try {
    const picked = await api.pickExecutableDialog()
    if (!picked) return
    try {
      const target = await api.resolveDroppedPath(picked)
      form.exe = target.exe || picked
      if (!form.name.trim() || !isEditMode.value) {
        form.name = target.name || extractFileStem(form.exe)
      }
      if (target.args) form.args = target.args
      if (target.cwd) form.cwd = target.cwd
      form.source = target.source || ''
      needsElevation.value = Boolean(target.needs_elevation)
      parseNote.value = target.parse_note || ''
      if (!isEditMode.value) {
        form.mode = guessMode(form.exe)
      }
    } catch {
      form.exe = picked
      if (!form.name.trim()) {
        form.name = extractFileStem(picked)
      }
      form.source = 'EXE'
    }
  } catch (err) {
    errorMsg.value = `打开文件选择器失败: ${err instanceof Error ? err.message : String(err)}`
  } finally {
    browsing.value = false
  }
}

function handleSave(createShortcutAfter: boolean) {
  errorMsg.value = ''
  const exe = form.exe.trim()
  if (!exe) {
    errorMsg.value = '请填写或浏览选择目标程序路径 (.exe / .lnk)。'
    return
  }
  if (props.ports.length > 0 && !form.port_id) {
    errorMsg.value = '请先选择一个绑定的代理端口。'
    return
  }

  const rule: Rule = {
    id: form.id || generateId(),
    name: form.name.trim() || extractFileStem(exe) || '未命名应用',
    exe,
    port_id: form.port_id,
    args: form.args.trim(),
    cwd: form.cwd.trim(),
    mode: form.mode,
    flag_template: form.flag_template.trim() || '--proxy-server={url}',
    enabled: form.enabled,
    source: form.source || (isEditMode.value ? props.editingRule?.source ?? '' : 'EXE'),
  }
  emit('save', rule, createShortcutAfter)
}

function onKeyDown(e: KeyboardEvent) {
  if (!props.visible) return
  if (e.key === 'Escape') {
    e.preventDefault()
    e.stopPropagation()
    emit('close')
  }
}

window.addEventListener('keydown', onKeyDown)
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeyDown)
})
</script>

<template>
  <Teleport to="body">
    <div
      v-if="visible"
      class="fixed inset-0 z-50 flex items-center justify-center p-4 backdrop-blur-[2px]"
      :class="isDark ? 'bg-black/65' : 'bg-slate-900/35'"
      @mousedown.self="emit('close')"
    >
      <div
        class="w-full max-w-[540px] rounded-[12px] border shadow-2xl overflow-hidden flex flex-col max-h-[90vh]"
        :class="
          isDark
            ? 'bg-[#1B1F28] border-[#2B3240] text-[#ECF0F8]'
            : 'bg-[#FFFFFF] border-[#D8E0EC] text-[#171B24]'
        "
        role="dialog"
        aria-modal="true"
        :aria-label="isEditMode ? '编辑规则' : '新建规则'"
      >
        <!-- Header -->
        <div
          class="flex items-center justify-between px-5 py-3.5 border-b shrink-0"
          :class="isDark ? 'border-[#252B38]' : 'border-[#E2E8F2]'"
        >
          <div class="flex items-center gap-2">
            <span
              class="inline-flex items-center justify-center w-6 h-6 rounded-md text-xs font-bold"
              :class="isDark ? 'bg-[#1A2B4C] text-[#60A5FA]' : 'bg-[#E0ECFF] text-[#2563EB]'"
            >
              <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 5v14m-7-7h14" v-if="!isEditMode" />
                <path stroke-linecap="round" stroke-linejoin="round" d="M16.862 3.487a2.1 2.1 0 1 1 2.97 2.97L7.5 18.79l-4 1 1-4L16.862 3.487Z" v-else />
              </svg>
            </span>
            <h2 class="text-[15px] font-semibold tracking-tight">
              {{ isEditMode ? '编辑启动规则' : '新建启动规则' }}
            </h2>
          </div>
          <button
            type="button"
            class="w-7 h-7 rounded-md inline-flex items-center justify-center transition-colors cursor-pointer"
            :class="isDark ? 'text-[#8B95A9] hover:bg-[#242A36] hover:text-[#ECF0F8]' : 'text-[#5C6678] hover:bg-[#EBF0F7] hover:text-[#171B24]'"
            title="关闭 (Esc)"
            @click="emit('close')"
          >
            <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        <!-- Form Body -->
        <form
          class="px-5 py-4 space-y-3.5 overflow-y-auto text-[13px]"
          @submit.prevent="handleSave(false)"
        >
          <!-- Elevation warning -->
          <div
            v-if="needsElevation"
            class="flex items-start gap-2.5 px-3 py-2.5 rounded-lg border text-xs leading-relaxed"
            :class="
              isDark
                ? 'bg-[#3A2810]/70 border-[#B45309]/50 text-[#FBBF24]'
                : 'bg-[#FEF3C7] border-[#F59E0B]/40 text-[#92400E]'
            "
          >
            <svg class="w-4 h-4 shrink-0 mt-0.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v4m0 4h.01M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0Z" />
            </svg>
            <span>此快捷方式要求管理员权限，普通权限下可能无法启动或注入代理环境变量。</span>
          </div>

          <!-- Error banner -->
          <div
            v-if="errorMsg"
            class="px-3 py-2 rounded-lg border text-xs"
            :class="
              isDark
                ? 'bg-[#3B181B] border-[#F87171]/40 text-[#FCA5A5]'
                : 'bg-[#FEE2E2] border-[#DC2626]/30 text-[#B91C1C]'
            "
          >
            {{ errorMsg }}
          </div>

          <!-- Name -->
          <div>
            <label
              class="block text-xs font-medium mb-1"
              :class="isDark ? 'text-[#8B95A9]' : 'text-[#5C6678]'"
            >
              规则名称
            </label>
            <input
              ref="nameInputRef"
              v-model="form.name"
              type="text"
              placeholder="例如 Chrome / Antigravity / Git"
              class="w-full h-9 px-3 rounded-lg border text-[13px] outline-none transition-colors"
              :class="
                isDark
                  ? 'bg-[#242A36] border-[#2B3240] text-[#ECF0F8] placeholder-[#5C6678] focus:border-[#60A5FA]'
                  : 'bg-[#F8FAFC] border-[#D8E0EC] text-[#171B24] placeholder-[#94A3B8] focus:border-[#3B82F6] focus:bg-white'
              "
            />
          </div>

          <!-- Executable path + Browse -->
          <div>
            <div class="flex items-center justify-between mb-1">
              <label
                class="text-xs font-medium"
                :class="isDark ? 'text-[#8B95A9]' : 'text-[#5C6678]'"
              >
                目标程序路径 (.exe / .lnk)
              </label>
              <span
                v-if="parseNote"
                class="text-[11px]"
                :class="isDark ? 'text-[#8B95A9]' : 'text-[#5C6678]'"
              >
                解析来源：{{ parseNote }}
              </span>
            </div>
            <div class="flex items-center gap-2">
              <input
                v-model="form.exe"
                type="text"
                placeholder="C:\Program Files\...\app.exe"
                class="flex-1 h-9 px-3 rounded-lg border text-xs font-mono outline-none transition-colors"
                :class="
                  isDark
                    ? 'bg-[#242A36] border-[#2B3240] text-[#ECF0F8] placeholder-[#5C6678] focus:border-[#60A5FA]'
                    : 'bg-[#F8FAFC] border-[#D8E0EC] text-[#171B24] placeholder-[#94A3B8] focus:border-[#3B82F6] focus:bg-white'
                "
              />
              <button
                type="button"
                :disabled="browsing"
                class="h-9 px-3.5 rounded-lg border text-xs font-medium transition-colors shrink-0 cursor-pointer disabled:opacity-50"
                :class="
                  isDark
                    ? 'border-[#2B3240] bg-[#242A36] text-[#ECF0F8] hover:bg-[#2E3748]'
                    : 'border-[#D8E0EC] bg-white text-[#171B24] hover:bg-[#EBF0F7]'
                "
                @click="handleBrowse"
              >
                {{ browsing ? '选择中…' : '浏览…' }}
              </button>
            </div>
          </div>

          <!-- Port Binding (native select = zero click-through bugs) -->
          <div>
            <label
              class="block text-xs font-medium mb-1"
              :class="isDark ? 'text-[#8B95A9]' : 'text-[#5C6678]'"
            >
              绑定代理端口
            </label>
            <div class="relative">
              <select
                v-model="form.port_id"
                :style="{ colorScheme: isDark ? 'dark' : 'light' }"
                class="w-full h-9 pl-3 pr-8 rounded-lg border text-[13px] outline-none appearance-none transition-colors cursor-pointer"
                :class="
                  isDark
                    ? 'bg-[#242A36] border-[#2B3240] text-[#ECF0F8] focus:border-[#60A5FA]'
                    : 'bg-[#F8FAFC] border-[#D8E0EC] text-[#171B24] focus:border-[#3B82F6] focus:bg-white'
                "
              >
                <option v-if="ports.length === 0" value="">（暂无端口，请先在左侧添加端口）</option>
                <option v-for="p in ports" :key="p.id" :value="p.id">
                  {{ p.alias || '未命名端口' }} ({{ p.host }}:{{ p.port }})
                </option>
              </select>
              <svg
                class="w-4 h-4 pointer-events-none absolute right-2.5 top-1/2 -translate-y-1/2"
                :class="isDark ? 'text-[#8B95A9]' : 'text-[#5C6678]'"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
              >
                <path stroke-linecap="round" stroke-linejoin="round" d="m6 9 6 6 6-6" />
              </svg>
            </div>
          </div>

          <!-- Args & Cwd -->
          <div class="grid grid-cols-2 gap-3">
            <div>
              <label
                class="block text-xs font-medium mb-1"
                :class="isDark ? 'text-[#8B95A9]' : 'text-[#5C6678]'"
              >
                启动参数 (可选)
              </label>
              <input
                v-model="form.args"
                type="text"
                placeholder="例如 --incognito"
                class="w-full h-9 px-3 rounded-lg border text-xs font-mono outline-none transition-colors"
                :class="
                  isDark
                    ? 'bg-[#242A36] border-[#2B3240] text-[#ECF0F8] placeholder-[#5C6678] focus:border-[#60A5FA]'
                    : 'bg-[#F8FAFC] border-[#D8E0EC] text-[#171B24] placeholder-[#94A3B8] focus:border-[#3B82F6] focus:bg-white'
                "
              />
            </div>
            <div>
              <label
                class="block text-xs font-medium mb-1"
                :class="isDark ? 'text-[#8B95A9]' : 'text-[#5C6678]'"
              >
                工作目录 (可选)
              </label>
              <input
                v-model="form.cwd"
                type="text"
                placeholder="留空默认程序所在目录"
                class="w-full h-9 px-3 rounded-lg border text-xs font-mono outline-none transition-colors"
                :class="
                  isDark
                    ? 'bg-[#242A36] border-[#2B3240] text-[#ECF0F8] placeholder-[#5C6678] focus:border-[#60A5FA]'
                    : 'bg-[#F8FAFC] border-[#D8E0EC] text-[#171B24] placeholder-[#94A3B8] focus:border-[#3B82F6] focus:bg-white'
                "
              />
            </div>
          </div>

          <!-- Antigravity CDP Injection Hint -->
          <p
            v-if="antigravityCdp && isAntigravityRule(form)"
            class="text-[11px] leading-snug px-2.5 py-1.5 rounded-md border"
            :class="
              isDark
                ? 'bg-[#1A2B4C]/40 border-[#3B82F6]/30 text-[#93C5FD]'
                : 'bg-[#EFF6FF] border-[#BFDBFE] text-[#1D4ED8]'
            "
          >
            已开启「CDP 远程调试接口」：启动此规则时将自动注入
            <code class="font-mono">--remote-debugging-port=0 --remote-debugging-address=127.0.0.1</code>
          </p>

          <!-- Injection Mode Cards -->
          <div>
            <label
              class="block text-xs font-medium mb-1.5"
              :class="isDark ? 'text-[#8B95A9]' : 'text-[#5C6678]'"
            >
              代理注入方式
            </label>
            <div class="grid grid-cols-3 gap-2">
              <button
                v-for="opt in MODE_OPTIONS"
                :key="opt.value"
                type="button"
                class="px-2.5 py-2 rounded-lg border text-left transition-all cursor-pointer"
                :class="
                  form.mode === opt.value
                    ? isDark
                      ? 'bg-[#1A2B4C] border-[#60A5FA] text-[#ECF0F8]'
                      : 'bg-[#E0ECFF] border-[#3B82F6] text-[#1D4ED8]'
                    : isDark
                      ? 'bg-[#242A36]/60 border-[#2B3240] text-[#8B95A9] hover:bg-[#242A36] hover:text-[#ECF0F8]'
                      : 'bg-[#F8FAFC] border-[#D8E0EC] text-[#5C6678] hover:bg-[#EBF0F7] hover:text-[#171B24]'
                "
                @click="form.mode = opt.value"
              >
                <div class="text-xs font-bold leading-tight">{{ opt.title }}</div>
                <div
                  class="text-[11px] mt-0.5 leading-tight opacity-85"
                  :class="form.mode === opt.value ? '' : 'opacity-75'"
                >
                  {{ opt.desc }}
                </div>
              </button>
            </div>
          </div>

          <!-- Flag Template & Enabled -->
          <div class="grid grid-cols-[1fr_auto] gap-3 items-end">
            <div>
              <label
                class="block text-xs font-medium mb-1"
                :class="isDark ? 'text-[#8B95A9]' : 'text-[#5C6678]'"
              >
                命令行参数模板
              </label>
              <input
                v-model="form.flag_template"
                type="text"
                placeholder="--proxy-server={url}"
                class="w-full h-9 px-3 rounded-lg border text-xs font-mono outline-none transition-colors"
                :class="[
                  isDark
                    ? 'bg-[#242A36] border-[#2B3240] text-[#ECF0F8] placeholder-[#5C6678] focus:border-[#60A5FA]'
                    : 'bg-[#F8FAFC] border-[#D8E0EC] text-[#171B24] placeholder-[#94A3B8] focus:border-[#3B82F6] focus:bg-white',
                  form.mode === 'env' ? 'opacity-60' : '',
                ]"
              />
            </div>

            <label
              class="h-9 px-3 rounded-lg border flex items-center gap-2 cursor-pointer select-none"
              :class="
                isDark
                  ? 'bg-[#242A36]/60 border-[#2B3240] text-[#ECF0F8]'
                  : 'bg-[#F8FAFC] border-[#D8E0EC] text-[#171B24]'
              "
            >
              <input
                v-model="form.enabled"
                type="checkbox"
                class="w-4 h-4 rounded accent-[#3B82F6] cursor-pointer"
              />
              <span class="text-xs font-medium">启用规则</span>
            </label>
          </div>

          <!-- Footer Buttons -->
          <div
            class="flex items-center justify-end gap-2 pt-3 mt-1 border-t"
            :class="isDark ? 'border-[#252B38]' : 'border-[#E2E8F2]'"
          >
            <button
              type="button"
              class="h-9 px-3.5 rounded-lg text-xs font-semibold inline-flex items-center gap-1.5 transition-colors cursor-pointer mr-auto"
              :class="
                isDark
                  ? 'bg-[#1E1B4B] text-[#818CF8] hover:bg-[#272361]'
                  : 'bg-[#E0E7FF] text-[#4F46E5] hover:bg-[#C7D2FE]'
              "
              @click="handleSave(true)"
            >
              <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M7 17 17 7M7 7h10v10" />
              </svg>
              <span>保存并建桌面快捷方式</span>
            </button>

            <button
              type="button"
              class="h-9 px-4 rounded-lg border text-xs font-medium transition-colors cursor-pointer"
              :class="
                isDark
                  ? 'border-[#2B3240] text-[#8B95A9] hover:bg-[#242A36] hover:text-[#ECF0F8]'
                  : 'border-[#D8E0EC] text-[#5C6678] hover:bg-[#EBF0F7] hover:text-[#171B24]'
              "
              @click="emit('close')"
            >
              取消
            </button>

            <button
              type="submit"
              class="h-9 px-5 rounded-lg text-xs font-semibold text-white transition-colors cursor-pointer"
              :class="isDark ? 'bg-[#3B82F6] hover:bg-[#60A5FA]' : 'bg-[#3B82F6] hover:bg-[#2563EB]'"
            >
              保存
            </button>
          </div>
        </form>
      </div>
    </div>
  </Teleport>
</template>
