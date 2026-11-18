<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import type { InjectionMode, Port, ProbeResult, Rule } from '../types'
import {
  getAvatarColor,
  getAvatarLetter,
  getModeStyle,
  getReasonStyle,
  isAntigravityRule,
} from '../types'

const props = defineProps<{
  rules: Rule[]
  ports: Port[]
  probeResults: Record<string, ProbeResult>
  selectedPortId: string | null
  antigravityCdp: boolean
  isDark: boolean
}>()

const emit = defineEmits<{
  'clear-port-filter': []
  'toggle-antigravity-cdp': []
  'add-rule': []
  'edit-rule': [rule: Rule]
  'delete-rules': [ruleIds: string[]]
  'update-rule': [rule: Rule]
  'batch-rebind-port': [ruleIds: string[], portId: string]
  'launch-rule': [rule: Rule]
  'batch-launch': [ruleIds: string[]]
  'create-shortcut': [rule: Rule]
  'batch-shortcut': [ruleIds: string[]]
  'files-dropped': [paths: string[]]
}>()

const MODE_ORDER: InjectionMode[] = ['env', 'flag', 'both']

const searchInputRef = ref<HTMLInputElement | null>(null)
const searchQuery = ref('')
const selectedIds = ref<string[]>([])
const dragDepth = ref(0)
const isDragOver = computed(() => dragDepth.value > 0)

// Brief border flash state when a rule's port is switched
const flashingIds = ref<Record<string, boolean>>({})
const flashTimers = new Map<string, number>()

function triggerFlash(ruleId: string) {
  const existing = flashTimers.get(ruleId)
  if (existing !== undefined) {
    window.clearTimeout(existing)
  }
  flashingIds.value = { ...flashingIds.value, [ruleId]: true }
  const timer = window.setTimeout(() => {
    const next = { ...flashingIds.value }
    delete next[ruleId]
    flashingIds.value = next
    flashTimers.delete(ruleId)
  }, 560)
  flashTimers.set(ruleId, timer)
}

// Keep selectedIds synced with existing rules
watch(
  () => props.rules,
  (list) => {
    const valid = new Set(list.map((r) => r.id))
    const filtered = selectedIds.value.filter((id) => valid.has(id))
    if (filtered.length !== selectedIds.value.length) {
      selectedIds.value = filtered
    }
  },
)

const portMap = computed(() => {
  const map = new Map<string, Port>()
  for (const p of props.ports) {
    map.set(p.id, p)
  }
  return map
})

const activeFilterPort = computed(() =>
  props.selectedPortId ? portMap.value.get(props.selectedPortId) ?? null : null,
)

const filteredRules = computed(() => {
  const q = searchQuery.value.trim().toLowerCase()
  return props.rules.filter((r) => {
    if (props.selectedPortId && r.port_id !== props.selectedPortId) {
      return false
    }
    if (!q) return true
    const port = portMap.value.get(r.port_id)
    const portLabel = port ? `${port.alias} ${port.host}:${port.port}`.toLowerCase() : ''
    return (
      r.name.toLowerCase().includes(q) ||
      r.exe.toLowerCase().includes(q) ||
      portLabel.includes(q)
    )
  })
})

const isFilterActive = computed(
  () => Boolean(props.selectedPortId) || searchQuery.value.trim().length > 0,
)

const countText = computed(() =>
  isFilterActive.value
    ? `${filteredRules.value.length}/${props.rules.length}`
    : `${props.rules.length}`,
)

const selectedSet = computed(() => new Set(selectedIds.value))
const selectedCount = computed(() => selectedIds.value.length)

const allVisibleSelected = computed(
  () =>
    filteredRules.value.length > 0 &&
    filteredRules.value.every((r) => selectedSet.value.has(r.id)),
)

const someVisibleSelected = computed(
  () =>
    !allVisibleSelected.value &&
    filteredRules.value.some((r) => selectedSet.value.has(r.id)),
)

function toggleSelectAll() {
  const visibleIds = filteredRules.value.map((r) => r.id)
  if (allVisibleSelected.value) {
    const visSet = new Set(visibleIds)
    selectedIds.value = selectedIds.value.filter((id) => !visSet.has(id))
  } else {
    const next = new Set(selectedIds.value)
    for (const id of visibleIds) next.add(id)
    selectedIds.value = Array.from(next)
  }
}

function toggleRuleSelection(ruleId: string) {
  if (selectedSet.value.has(ruleId)) {
    selectedIds.value = selectedIds.value.filter((id) => id !== ruleId)
  } else {
    selectedIds.value = [...selectedIds.value, ruleId]
  }
}

function clearSelection() {
  selectedIds.value = []
}

function getSelectedIds(): string[] {
  const valid = new Set(props.rules.map((r) => r.id))
  return selectedIds.value.filter((id) => valid.has(id))
}

function focusSearch() {
  searchInputRef.value?.focus()
  searchInputRef.value?.select()
}

defineExpose({
  focusSearch,
  getSelectedIds,
  clearSelection,
})

// Helper display utilities
function getRuleDisplayName(rule: Rule): string {
  if (rule.name.trim()) return rule.name.trim()
  const norm = rule.exe.replace(/\//g, '\\')
  const base = norm.split('\\').pop() || ''
  return base || '(未命名)'
}

function shortenPath(pathStr: string, maxLen = 54): string {
  if (!pathStr) return ''
  if (pathStr.length <= maxLen) return pathStr
  const parts = pathStr.replace(/\//g, '\\').split('\\').filter(Boolean)
  if (parts.length <= 2) return pathStr
  const head = parts[0]
  const tail = parts.slice(-2).join('\\')
  return `${head}\\...\\${tail}`
}

function getPortDotColor(portId: string): string {
  const res = props.probeResults[portId]
  if (!res) return props.isDark ? '#8B95A9' : '#5C6678'
  return getReasonStyle(res.reason, props.isDark).fg
}

function getPortBadgeInfo(rule: Rule): { label: string; dotColor: string; unbound: boolean } {
  const port = portMap.value.get(rule.port_id)
  if (!port) {
    return {
      label: '未绑定',
      dotColor: props.isDark ? '#F87171' : '#DC2626',
      unbound: true,
    }
  }
  return {
    label: port.alias || `${port.port}`,
    dotColor: getPortDotColor(port.id),
    unbound: false,
  }
}

function getModePill(mode: string) {
  return getModeStyle(mode, props.isDark)
}

function cycleMode(rule: Rule) {
  const idx = MODE_ORDER.indexOf(rule.mode)
  const next = MODE_ORDER[(idx + 1) % MODE_ORDER.length]
  emit('update-rule', { ...rule, mode: next })
}

function setRuleMode(rule: Rule, mode: InjectionMode) {
  if (rule.mode !== mode) {
    emit('update-rule', { ...rule, mode })
  }
}

function toggleEnabled(rule: Rule) {
  emit('update-rule', { ...rule, enabled: !rule.enabled })
}

function selectPortForRule(rule: Rule, portId: string) {
  if (rule.port_id !== portId) {
    emit('update-rule', { ...rule, port_id: portId })
  }
  triggerFlash(rule.id)
}

function cyclePort(rule: Rule) {
  if (props.ports.length === 0) return
  const idx = props.ports.findIndex((p) => p.id === rule.port_id)
  const nextPort = props.ports[(idx + 1) % props.ports.length]
  if (nextPort) {
    selectPortForRule(rule, nextPort.id)
  }
}

// Floating Port Picker Menu (single rule or batch)
interface PortMenuState {
  mode: 'single' | 'batch'
  rule: Rule | null
  x: number
  y: number
  width: number
}

const portMenu = ref<PortMenuState | null>(null)

// Row Context Menu
interface ContextMenuState {
  rule: Rule
  x: number
  y: number
  submenu: 'port' | 'mode' | null
}

const contextMenu = ref<ContextMenuState | null>(null)

function closeAllMenus() {
  portMenu.value = null
  contextMenu.value = null
}

function computeMenuCoords(anchorRect: DOMRect, menuWidth: number, itemCount: number) {
  const estHeight = Math.min(320, Math.max(48, itemCount * 34 + 12))
  let x = Math.min(anchorRect.left, window.innerWidth - menuWidth - 8)
  x = Math.max(8, x)
  let y = anchorRect.bottom + 6
  if (y + estHeight > window.innerHeight - 8) {
    const yAbove = anchorRect.top - estHeight - 6
    y = yAbove >= 8 ? yAbove : Math.max(8, window.innerHeight - estHeight - 8)
  }
  return { x, y, width: menuWidth }
}

function openSinglePortMenu(e: MouseEvent, rule: Rule) {
  contextMenu.value = null
  if (portMenu.value?.mode === 'single' && portMenu.value.rule?.id === rule.id) {
    portMenu.value = null
    return
  }
  const el = e.currentTarget as HTMLElement
  const rect = el.getBoundingClientRect()
  const pos = computeMenuCoords(rect, Math.max(240, rect.width + 48), props.ports.length)
  portMenu.value = { mode: 'single', rule, ...pos }
}

function openBatchPortMenu(e: MouseEvent) {
  contextMenu.value = null
  if (selectedCount.value === 0) return
  if (portMenu.value?.mode === 'batch') {
    portMenu.value = null
    return
  }
  const el = e.currentTarget as HTMLElement
  const rect = el.getBoundingClientRect()
  const pos = computeMenuCoords(rect, 252, props.ports.length)
  portMenu.value = { mode: 'batch', rule: null, ...pos }
}

function handlePortMenuPick(portId: string) {
  if (!portMenu.value) return
  if (portMenu.value.mode === 'single' && portMenu.value.rule) {
    selectPortForRule(portMenu.value.rule, portId)
  } else if (portMenu.value.mode === 'batch') {
    const ids = getSelectedIds()
    if (ids.length > 0) {
      emit('batch-rebind-port', ids, portId)
      for (const id of ids) triggerFlash(id)
    }
  }
  portMenu.value = null
}

function openContextMenu(e: MouseEvent, rule: Rule) {
  portMenu.value = null
  const menuW = 204
  const menuH = 236
  const x = Math.max(8, Math.min(e.clientX, window.innerWidth - menuW - 8))
  const y = Math.max(8, Math.min(e.clientY, window.innerHeight - menuH - 8))
  contextMenu.value = { rule, x, y, submenu: null }
}

const submenuFlipLeft = computed(() => {
  if (!contextMenu.value) return false
  return contextMenu.value.x + 204 + 236 > window.innerWidth - 8
})

// HTML5 fallback drag-and-drop
function onDragEnter(e: DragEvent) {
  e.preventDefault()
  dragDepth.value += 1
}

function onDragOver(e: DragEvent) {
  e.preventDefault()
  if (e.dataTransfer) {
    e.dataTransfer.dropEffect = 'copy'
  }
}

function onDragLeave(e: DragEvent) {
  e.preventDefault()
  dragDepth.value = Math.max(0, dragDepth.value - 1)
}

function onDrop(e: DragEvent) {
  e.preventDefault()
  dragDepth.value = 0
  const paths: string[] = []
  const files = e.dataTransfer?.files
  if (files && files.length > 0) {
    for (let i = 0; i < files.length; i += 1) {
      const f = files[i] as File & { path?: string }
      if (f?.path) {
        paths.push(f.path)
      }
    }
  }
  if (paths.length > 0) {
    emit('files-dropped', paths)
  }
}

function handleGlobalPointerDown(e: MouseEvent) {
  const target = e.target as HTMLElement | null
  if (target?.closest('[data-floating-menu="true"]')) return
  closeAllMenus()
}

function handleGlobalKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    closeAllMenus()
  }
}

window.addEventListener('mousedown', handleGlobalPointerDown)
window.addEventListener('keydown', handleGlobalKeyDown)
window.addEventListener('blur', closeAllMenus)
window.addEventListener('resize', closeAllMenus)

onBeforeUnmount(() => {
  window.removeEventListener('mousedown', handleGlobalPointerDown)
  window.removeEventListener('keydown', handleGlobalKeyDown)
  window.removeEventListener('blur', closeAllMenus)
  window.removeEventListener('resize', closeAllMenus)
  for (const t of flashTimers.values()) {
    window.clearTimeout(t)
  }
  flashTimers.clear()
})
</script>

<template>
  <section
    class="flex flex-col h-full min-w-0 select-none"
    :class="isDark ? 'bg-[#12151B] text-[#ECF0F8]' : 'bg-[#F3F5F9] text-[#171B24]'"
  >
    <!-- Header / Filter Bar -->
    <header class="flex items-center gap-2.5 px-4 pt-3.5 pb-2.5 shrink-0 flex-wrap">
      <div class="flex items-center gap-2 mr-1">
        <h1 class="text-[18px] font-bold tracking-tight leading-none">
          Rules · 启动规则
        </h1>
        <span
          class="text-xs font-semibold px-2 py-0.5 rounded-full tabular-nums"
          :class="isDark ? 'bg-[#242A36] text-[#8B95A9]' : 'bg-[#EBF0F7] text-[#5C6678]'"
        >
          {{ countText }}
        </span>
      </div>

      <!-- Active Port Filter Chip -->
      <button
        v-if="selectedPortId"
        type="button"
        class="h-[26px] px-2.5 rounded-full text-[11px] font-bold inline-flex items-center gap-1.5 transition-colors cursor-pointer"
        :class="
          isDark
            ? 'bg-[#1A2B4C] text-[#93C5FD] hover:bg-[#243B66]'
            : 'bg-[#DBEAFE] text-[#1D4ED8] hover:bg-[#BFDBFE]'
        "
        title="点击清除端口筛选，显示全部规则"
        @click="emit('clear-port-filter')"
      >
        <span
          class="w-2 h-2 rounded-full shrink-0"
          :style="{ backgroundColor: getPortDotColor(selectedPortId) }"
        />
        <span>
          端口:
          {{
            activeFilterPort
              ? activeFilterPort.alias || `${activeFilterPort.host}:${activeFilterPort.port}`
              : '未知'
          }}
        </span>
        <span class="opacity-75 ml-0.5">✕</span>
      </button>

      <!-- CDP Toggle + Search + Select All (right side) -->
      <div class="ml-auto flex items-center gap-2 flex-wrap">
        <button
          type="button"
          role="switch"
          :aria-checked="antigravityCdp"
          class="h-8 px-2.5 rounded-lg border text-xs font-medium inline-flex items-center gap-1.5 transition-colors cursor-pointer shrink-0"
          :class="
            antigravityCdp
              ? isDark
                ? 'bg-[#1A2B4C]/75 border-[#3B82F6]/60 text-[#ECF0F8]'
                : 'bg-[#EFF6FF] border-[#93C5FD] text-[#1D4ED8]'
              : isDark
                ? 'bg-[#1B1F28] border-[#2B3240] text-[#8B95A9] hover:bg-[#242A36] hover:text-[#ECF0F8]'
                : 'bg-white border-[#D8E0EC] text-[#5C6678] hover:bg-[#EBF0F7] hover:text-[#171B24]'
          "
          title="开启后，仅对 Antigravity (agy) 生效：在软件内打开或用生成的桌面快捷方式启动时自动注入 --remote-debugging-port=0 --remote-debugging-address=127.0.0.1"
          @click="emit('toggle-antigravity-cdp')"
        >
          <span>CDP 远程调试接口</span>
          <span
            class="px-1.5 py-[1px] rounded text-[10px] font-semibold leading-tight"
            :class="
              antigravityCdp
                ? isDark
                  ? 'bg-[#243B66] text-[#93C5FD]'
                  : 'bg-[#DBEAFE] text-[#1D4ED8]'
                : isDark
                  ? 'bg-[#242A36] text-[#8B95A9]'
                  : 'bg-[#EBF0F7] text-[#5C6678]'
            "
          >
            仅 Antigravity
          </span>
          <span
            class="relative w-7 h-4 rounded-full transition-colors shrink-0 ml-0.5"
            :class="
              antigravityCdp
                ? 'bg-[#3B82F6]'
                : isDark
                  ? 'bg-[#2B3240]'
                  : 'bg-[#CBD5E1]'
            "
          >
            <span
              class="absolute top-0.5 left-0.5 w-3 h-3 rounded-full bg-white shadow transition-transform"
              :class="antigravityCdp ? 'translate-x-3' : 'translate-x-0'"
            />
          </span>
        </button>

        <div class="relative flex items-center">
          <svg
            class="w-3.5 h-3.5 absolute left-2.5 pointer-events-none"
            :class="isDark ? 'text-[#8B95A9]' : 'text-[#5C6678]'"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <circle cx="11" cy="11" r="8" />
            <path stroke-linecap="round" stroke-linejoin="round" d="m21 21-4.35-4.35" />
          </svg>
          <input
            ref="searchInputRef"
            v-model="searchQuery"
            type="text"
            placeholder="搜索规则… (Ctrl+F)"
            class="h-8 w-[180px] pl-8 pr-7 rounded-lg border text-xs outline-none transition-colors"
            :class="
              isDark
                ? 'bg-[#1B1F28] border-[#2B3240] text-[#ECF0F8] placeholder-[#5C6678] focus:border-[#60A5FA]'
                : 'bg-white border-[#D8E0EC] text-[#171B24] placeholder-[#94A3B8] focus:border-[#3B82F6]'
            "
          />
          <button
            v-if="searchQuery"
            type="button"
            class="absolute right-2 w-4 h-4 rounded-full flex items-center justify-center text-[11px] cursor-pointer"
            :class="isDark ? 'text-[#8B95A9] hover:text-[#ECF0F8]' : 'text-[#5C6678] hover:text-[#171B24]'"
            title="清空搜索"
            @click="searchQuery = ''"
          >
            ✕
          </button>
        </div>

        <label
          class="h-8 px-2.5 rounded-lg border text-xs font-medium inline-flex items-center gap-1.5 cursor-pointer transition-colors"
          :class="
            isDark
              ? 'border-[#2B3240] text-[#8B95A9] hover:bg-[#242A36] hover:text-[#ECF0F8]'
              : 'border-[#D8E0EC] bg-white text-[#5C6678] hover:bg-[#EBF0F7] hover:text-[#171B24]'
          "
          title="全选或取消全选当前可见规则"
        >
          <input
            type="checkbox"
            :checked="allVisibleSelected"
            :indeterminate.prop="someVisibleSelected"
            :disabled="filteredRules.length === 0"
            class="w-3.5 h-3.5 rounded accent-[#3B82F6] cursor-pointer"
            @change="toggleSelectAll"
          />
          <span>全选</span>
        </label>
      </div>
    </header>

    <!-- Top Drag-and-Drop Zone (~72px) -->
    <div class="px-4 pb-2.5 shrink-0">
      <div
        role="button"
        tabindex="0"
        class="h-[72px] rounded-[10px] border-2 border-dashed flex items-center justify-center gap-3 px-4 transition-all cursor-pointer group"
        :class="
          isDragOver
            ? isDark
              ? 'border-[#60A5FA] bg-[#1A2B4C]/70 text-[#93C5FD] scale-[1.004]'
              : 'border-[#3B82F6] bg-[#E0ECFF]/80 text-[#1D4ED8] scale-[1.004]'
            : isDark
              ? 'border-[#2B3240] bg-[#161A22]/70 text-[#8B95A9] hover:border-[#60A5FA]/70 hover:bg-[#1A2B4C]/35 hover:text-[#ECF0F8]'
              : 'border-[#CBD5E1] bg-white/60 text-[#5C6678] hover:border-[#3B82F6]/70 hover:bg-[#E0ECFF]/45 hover:text-[#1D4ED8]'
        "
        @click="emit('add-rule')"
        @keydown.enter.prevent="emit('add-rule')"
        @dragenter="onDragEnter"
        @dragover="onDragOver"
        @dragleave="onDragLeave"
        @drop="onDrop"
      >
        <svg
          class="w-5 h-5 shrink-0 transition-transform group-hover:scale-110"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.9"
        >
          <path stroke-linecap="round" stroke-linejoin="round" d="M12 16V4m0 0-4 4m4-4 4 4" />
          <path stroke-linecap="round" stroke-linejoin="round" d="M4 17v2a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-2" />
        </svg>
        <span class="text-[13px] font-medium truncate">
          {{
            isDragOver
              ? '松开即可自动解析 .exe / .lnk 并添加规则'
              : '将 .exe 或桌面快捷方式 (.lnk) 拖到这里，或点击'
          }}
        </span>
        <button
          v-if="!isDragOver"
          type="button"
          class="h-7 px-3 rounded-md text-xs font-semibold shrink-0 transition-colors cursor-pointer"
          :class="
            isDark
              ? 'bg-[#1A2B4C] text-[#93C5FD] hover:bg-[#243B66]'
              : 'bg-[#DBEAFE] text-[#1D4ED8] hover:bg-[#BFDBFE]'
          "
          @click.stop="emit('add-rule')"
        >
          + 添加规则
        </button>
      </div>
    </div>

    <!-- Scrollable Rule List -->
    <div
      class="flex-1 overflow-y-auto px-4 pb-2 space-y-1.5"
      @scroll="closeAllMenus"
    >
      <div
        v-if="filteredRules.length === 0"
        class="h-44 flex flex-col items-center justify-center text-center px-6 rounded-[10px] border border-dashed"
        :class="
          isDark
            ? 'border-[#252B38] text-[#8B95A9]'
            : 'border-[#D8E0EC] text-[#5C6678]'
        "
      >
        <p class="text-[13px]">
          {{
            selectedPortId
              ? '该端口下暂无绑定规则（可点击上方筛选标签查看全部）'
              : searchQuery.trim()
                ? '没有匹配搜索条件的规则'
                : '把 .lnk 或 .exe 拖到上方虚线框，建立第一条规则'
          }}
        </p>
      </div>

      <div
        v-for="rule in filteredRules"
        :key="rule.id"
        class="min-h-[60px] px-2.5 py-2 rounded-[10px] border flex items-center gap-2.5 transition-all"
        :class="[
          flashingIds[rule.id] ? 'port-switch-flash' : '',
          isDark
            ? 'bg-[#1B1F28] border-[#252B38] hover:bg-[#212734] hover:border-[#456B9E]'
            : 'bg-[#FFFFFF] border-[#E2E8F2] hover:bg-[#F6F9FD] hover:border-[#94BCEB]',
        ]"
        @dblclick="rule.enabled && emit('launch-rule', rule)"
        @contextmenu.prevent="openContextMenu($event, rule)"
      >
        <!-- Checkbox -->
        <div class="pl-0.5 flex items-center shrink-0" @click.stop @dblclick.stop>
          <input
            type="checkbox"
            :checked="selectedSet.has(rule.id)"
            class="w-4 h-4 rounded accent-[#3B82F6] cursor-pointer"
            @change="toggleRuleSelection(rule.id)"
          />
        </div>

        <!-- Deterministic Avatar -->
        <div
          class="w-9 h-9 rounded-[8px] shrink-0 flex items-center justify-center text-white font-bold text-[15px] shadow-xs"
          :style="{ backgroundColor: getAvatarColor(getRuleDisplayName(rule)) }"
        >
          {{ getAvatarLetter(getRuleDisplayName(rule)) }}
        </div>

        <!-- Name + Source Tag + Path -->
        <div class="flex-1 min-w-0">
          <div class="flex items-center gap-1.5 min-w-0">
            <span
              class="text-[13.5px] font-bold truncate"
              :class="
                rule.enabled
                  ? isDark
                    ? 'text-[#ECF0F8]'
                    : 'text-[#171B24]'
                  : isDark
                    ? 'text-[#8B95A9]'
                    : 'text-[#5C6678]'
              "
            >
              {{ getRuleDisplayName(rule) }}
            </span>
            <span
              v-if="rule.source"
              class="px-1.5 py-[1px] rounded text-[10px] font-bold shrink-0"
              :class="isDark ? 'bg-[#242A36] text-[#8B95A9]' : 'bg-[#EBF0F7] text-[#5C6678]'"
            >
              {{ rule.source }}
            </span>
            <span
              v-if="antigravityCdp && isAntigravityRule(rule)"
              class="px-1.5 py-[1px] rounded text-[10px] font-bold shrink-0"
              :class="isDark ? 'bg-[#1A2B4C] text-[#60A5FA]' : 'bg-[#DBEAFE] text-[#1D4ED8]'"
              title="已开启 CDP 远程调试接口：启动时将自动注入 --remote-debugging-port=0 --remote-debugging-address=127.0.0.1"
            >
              CDP
            </span>
          </div>
          <div
            class="text-[11px] font-mono truncate mt-0.5"
            :class="isDark ? 'text-[#8B95A9]' : 'text-[#5C6678]'"
            :title="rule.exe"
          >
            {{ shortenPath(rule.exe) || '（未设置路径）' }}
          </div>
        </div>

        <!-- Interactive PortBadge Pill -->
        <button
          type="button"
          data-floating-menu="true"
          class="h-6 px-2.5 rounded-md text-[11px] font-bold inline-flex items-center gap-1.5 shrink-0 transition-colors cursor-pointer"
          :class="
            isDark
              ? 'bg-[#242A36] hover:bg-[#2E3748] text-[#ECF0F8]'
              : 'bg-[#EBF0F7] hover:bg-[#DCE7F7] text-[#171B24]'
          "
          title="点击选择代理端口 · 右键快速切换下一个端口"
          @click.stop="openSinglePortMenu($event, rule)"
          @dblclick.stop
          @contextmenu.prevent.stop="cyclePort(rule)"
        >
          <span
            class="w-2 h-2 rounded-full shrink-0"
            :style="{ backgroundColor: getPortBadgeInfo(rule).dotColor }"
          />
          <span
            class="max-w-[96px] truncate"
            :class="
              getPortBadgeInfo(rule).unbound
                ? isDark
                  ? 'text-[#F87171]'
                  : 'text-[#DC2626]'
                : ''
            "
          >
            {{ getPortBadgeInfo(rule).label }}
          </span>
          <span class="opacity-65 text-[10px]">▾</span>
        </button>

        <!-- Injection Mode Pill -->
        <button
          type="button"
          class="h-[22px] px-2 rounded-md text-[11px] font-bold shrink-0 transition-opacity hover:opacity-85 cursor-pointer"
          :class="isDark ? 'bg-[#242A36]' : 'bg-[#EBF0F7]'"
          :style="{ color: getModePill(rule.mode).color }"
          title="点击循环切换注入方式 (env / flag / both)"
          @click.stop="cycleMode(rule)"
          @dblclick.stop
        >
          {{ getModePill(rule.mode).label }}
        </button>

        <!-- Enable Toggle Switch -->
        <button
          type="button"
          role="switch"
          :aria-checked="rule.enabled"
          class="relative w-9 h-5 rounded-full transition-colors shrink-0 cursor-pointer"
          :class="
            rule.enabled
              ? 'bg-[#3B82F6]'
              : isDark
                ? 'bg-[#2B3240]'
                : 'bg-[#CBD5E1]'
          "
          :title="rule.enabled ? '已启用（点击停用）' : '已停用（点击启用）'"
          @click.stop="toggleEnabled(rule)"
          @dblclick.stop
        >
          <span
            class="absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-white shadow transition-transform"
            :class="rule.enabled ? 'translate-x-4' : 'translate-x-0'"
          />
        </button>

        <!-- Action Buttons -->
        <div class="flex items-center gap-0.5 shrink-0" @click.stop @dblclick.stop>
          <button
            type="button"
            :disabled="!rule.enabled"
            class="h-7 px-2 rounded-lg text-xs font-bold inline-flex items-center gap-1 transition-colors cursor-pointer disabled:opacity-35 disabled:cursor-not-allowed"
            :class="
              isDark
                ? 'text-[#60A5FA] hover:bg-[#242A36]'
                : 'text-[#3B82F6] hover:bg-[#EBF0F7]'
            "
            title="带代理启动 (双击行也可启动)"
            @click="emit('launch-rule', rule)"
          >
            <svg class="w-3 h-3 fill-current" viewBox="0 0 24 24">
              <path d="M8 5v14l11-7z" />
            </svg>
            <span>启动</span>
          </button>

          <button
            type="button"
            class="w-7 h-7 rounded-lg inline-flex items-center justify-center transition-colors cursor-pointer"
            :class="
              isDark
                ? 'text-[#818CF8] hover:bg-[#242A36]'
                : 'text-[#4F46E5] hover:bg-[#EBF0F7]'
            "
            title="生成桌面直连图标"
            @click="emit('create-shortcut', rule)"
          >
            <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.3">
              <path stroke-linecap="round" stroke-linejoin="round" d="M7 17 17 7M7 7h10v10" />
            </svg>
          </button>

          <button
            type="button"
            class="h-7 px-2 rounded-lg text-[11px] font-bold transition-colors cursor-pointer"
            :class="
              isDark
                ? 'text-[#8B95A9] hover:bg-[#242A36] hover:text-[#ECF0F8]'
                : 'text-[#5C6678] hover:bg-[#EBF0F7] hover:text-[#171B24]'
            "
            title="编辑规则"
            @click="emit('edit-rule', rule)"
          >
            编辑
          </button>

          <button
            type="button"
            class="w-7 h-7 rounded-lg inline-flex items-center justify-center text-xs font-bold transition-colors cursor-pointer"
            :class="
              isDark
                ? 'text-[#F87171] hover:bg-[#3B181B]'
                : 'text-[#DC2626] hover:bg-[#FEE2E2]'
            "
            title="删除规则"
            @click="emit('delete-rules', [rule.id])"
          >
            ✕
          </button>
        </div>
      </div>
    </div>

    <!-- Bottom Batch Action Bar (~64px) -->
    <footer
      class="h-[64px] px-4 border-t flex items-center justify-between gap-3 shrink-0"
      :class="
        isDark
          ? 'bg-[#12151B] border-[#252B38]'
          : 'bg-[#F3F5F9] border-[#E2E8F2]'
      "
    >
      <div class="flex items-center gap-2.5 text-xs">
        <span :class="isDark ? 'text-[#8B95A9]' : 'text-[#5C6678]'">
          已选 <strong :class="isDark ? 'text-[#ECF0F8]' : 'text-[#171B24]'">{{ selectedCount }}</strong> 项
        </span>
        <button
          v-if="selectedCount > 0"
          type="button"
          class="px-2 py-1 rounded-md font-medium transition-colors cursor-pointer"
          :class="
            isDark
              ? 'text-[#60A5FA] hover:bg-[#242A36]'
              : 'text-[#3B82F6] hover:bg-[#EBF0F7]'
          "
          @click="clearSelection"
        >
          取消选择
        </button>
      </div>

      <div class="flex items-center gap-2">
        <button
          type="button"
          data-floating-menu="true"
          :disabled="selectedCount === 0"
          class="h-9 px-3 rounded-lg border text-xs font-semibold inline-flex items-center gap-1 transition-colors cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
          :class="
            isDark
              ? 'bg-[#242A36] border-[#2B3240] text-[#ECF0F8] hover:bg-[#2E3748]'
              : 'bg-[#EBF0F7] border-[#D8E0EC] text-[#171B24] hover:bg-[#DCE7F7]'
          "
          title="批量切换所选规则绑定的代理端口"
          @click="openBatchPortMenu"
        >
          <span>⇄ 切换端口 ▾</span>
        </button>

        <button
          type="button"
          :disabled="selectedCount === 0"
          class="h-9 px-3.5 rounded-lg text-xs font-semibold inline-flex items-center gap-1.5 transition-colors cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
          :class="
            isDark
              ? 'bg-[#1E1B4B] text-[#818CF8] hover:bg-[#272361]'
              : 'bg-[#E0E7FF] text-[#4F46E5] hover:bg-[#C7D2FE]'
          "
          title="为所选规则批量生成桌面直连快捷方式"
          @click="emit('batch-shortcut', getSelectedIds())"
        >
          <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M7 17 17 7M7 7h10v10" />
          </svg>
          <span>生成桌面直连图标</span>
        </button>

        <button
          type="button"
          :disabled="selectedCount === 0"
          class="h-9 px-3.5 rounded-lg text-xs font-semibold text-white inline-flex items-center gap-1.5 transition-colors cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
          :class="
            isDark
              ? 'bg-[#3B82F6] hover:bg-[#60A5FA]'
              : 'bg-[#3B82F6] hover:bg-[#2563EB]'
          "
          title="批量启动所选已启用规则 (Ctrl+Enter)"
          @click="emit('batch-launch', getSelectedIds())"
        >
          <svg class="w-3.5 h-3.5 fill-current" viewBox="0 0 24 24">
            <path d="M8 5v14l11-7z" />
          </svg>
          <span>启动所选</span>
        </button>

        <button
          type="button"
          :disabled="selectedCount === 0"
          class="h-9 px-3 rounded-lg text-xs font-semibold transition-colors cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
          :class="
            isDark
              ? 'bg-[#3B181B] text-[#F87171] hover:bg-[#4C1D21]'
              : 'bg-[#FEE2E2] text-[#DC2626] hover:bg-[#FECACA]'
          "
          title="删除所选规则 (Delete)"
          @click="emit('delete-rules', getSelectedIds())"
        >
          删除所选
        </button>
      </div>
    </footer>

    <!-- Floating Quick Port Picker Menu -->
    <Teleport to="body">
      <div
        v-if="portMenu"
        data-floating-menu="true"
        class="fixed z-50 rounded-lg border p-1 shadow-xl"
        :style="{
          left: `${portMenu.x}px`,
          top: `${portMenu.y}px`,
          width: `${portMenu.width}px`,
        }"
        :class="
          isDark
            ? 'bg-[#1B1F28] border-[#60A5FA] text-[#ECF0F8]'
            : 'bg-[#FFFFFF] border-[#3B82F6] text-[#171B24]'
        "
      >
        <div
          v-if="ports.length === 0"
          class="px-3 py-2 text-xs"
          :class="isDark ? 'text-[#8B95A9]' : 'text-[#5C6678]'"
        >
          暂无可用代理端口
        </div>
        <button
          v-for="p in ports"
          :key="p.id"
          type="button"
          class="w-full h-8 px-2.5 rounded-md flex items-center gap-2 text-xs text-left transition-colors cursor-pointer"
          :class="
            portMenu.mode === 'single' && portMenu.rule?.port_id === p.id
              ? isDark
                ? 'bg-[#1A2B4C] font-semibold'
                : 'bg-[#E0ECFF] font-semibold'
              : isDark
                ? 'hover:bg-[#252D3D]'
                : 'hover:bg-[#EEF4FF]'
          "
          @click="handlePortMenuPick(p.id)"
        >
          <span
            class="w-2 h-2 rounded-full shrink-0"
            :style="{ backgroundColor: getPortDotColor(p.id) }"
          />
          <span class="flex-1 truncate">
            {{ p.alias || '端口' }} · {{ p.host }}:{{ p.port }}
          </span>
          <span
            v-if="portMenu.mode === 'single' && portMenu.rule?.port_id === p.id"
            class="text-xs font-bold"
            :class="isDark ? 'text-[#60A5FA]' : 'text-[#3B82F6]'"
          >
            ✓
          </span>
        </button>
      </div>
    </Teleport>

    <!-- Floating Row Context Menu -->
    <Teleport to="body">
      <div
        v-if="contextMenu"
        data-floating-menu="true"
        class="fixed z-50 w-[204px] rounded-lg border p-1 shadow-xl text-xs"
        :style="{ left: `${contextMenu.x}px`, top: `${contextMenu.y}px` }"
        :class="
          isDark
            ? 'bg-[#1B1F28] border-[#2B3240] text-[#ECF0F8]'
            : 'bg-[#FFFFFF] border-[#D8E0EC] text-[#171B24]'
        "
      >
        <button
          type="button"
          :disabled="!contextMenu.rule.enabled"
          class="w-full h-8 px-2.5 rounded-md flex items-center gap-2 text-left transition-colors cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
          :class="isDark ? 'hover:bg-[#252D3D]' : 'hover:bg-[#EEF4FF]'"
          @click="emit('launch-rule', contextMenu.rule); closeAllMenus()"
        >
          <span class="w-4 text-center">▶</span>
          <span>启动</span>
        </button>

        <!-- Submenu: 切换代理端口 -->
        <div
          class="relative"
          @mouseenter="contextMenu.submenu = 'port'"
        >
          <button
            type="button"
            class="w-full h-8 px-2.5 rounded-md flex items-center gap-2 text-left transition-colors cursor-pointer"
            :class="
              contextMenu.submenu === 'port'
                ? isDark
                  ? 'bg-[#252D3D]'
                  : 'bg-[#EEF4FF]'
                : isDark
                  ? 'hover:bg-[#252D3D]'
                  : 'hover:bg-[#EEF4FF]'
            "
            @click="contextMenu.submenu = contextMenu.submenu === 'port' ? null : 'port'"
          >
            <span class="w-4 text-center">⇄</span>
            <span class="flex-1">切换代理端口</span>
            <span class="opacity-60">▸</span>
          </button>

          <div
            v-if="contextMenu.submenu === 'port'"
            class="absolute top-0 w-[232px] rounded-lg border p-1 shadow-xl z-10"
            :class="[
              submenuFlipLeft ? 'right-full mr-1' : 'left-full ml-1',
              isDark
                ? 'bg-[#1B1F28] border-[#2B3240] text-[#ECF0F8]'
                : 'bg-[#FFFFFF] border-[#D8E0EC] text-[#171B24]',
            ]"
          >
            <div
              v-if="ports.length === 0"
              class="px-2.5 py-1.5"
              :class="isDark ? 'text-[#8B95A9]' : 'text-[#5C6678]'"
            >
              （暂无端口）
            </div>
            <button
              v-for="p in ports"
              :key="p.id"
              type="button"
              class="w-full h-8 px-2.5 rounded-md flex items-center gap-2 text-left transition-colors cursor-pointer"
              :class="
                contextMenu.rule.port_id === p.id
                  ? isDark
                    ? 'bg-[#1A2B4C] font-semibold'
                    : 'bg-[#E0ECFF] font-semibold'
                  : isDark
                    ? 'hover:bg-[#252D3D]'
                    : 'hover:bg-[#EEF4FF]'
              "
              @click="selectPortForRule(contextMenu.rule, p.id); closeAllMenus()"
            >
              <span
                class="w-2 h-2 rounded-full shrink-0"
                :style="{ backgroundColor: getPortDotColor(p.id) }"
              />
              <span class="flex-1 truncate">{{ p.alias || '端口' }} ({{ p.host }}:{{ p.port }})</span>
              <span v-if="contextMenu.rule.port_id === p.id" class="font-bold">✓</span>
            </button>
          </div>
        </div>

        <!-- Submenu: 切换注入方式 -->
        <div
          class="relative"
          @mouseenter="contextMenu.submenu = 'mode'"
        >
          <button
            type="button"
            class="w-full h-8 px-2.5 rounded-md flex items-center gap-2 text-left transition-colors cursor-pointer"
            :class="
              contextMenu.submenu === 'mode'
                ? isDark
                  ? 'bg-[#252D3D]'
                  : 'bg-[#EEF4FF]'
                : isDark
                  ? 'hover:bg-[#252D3D]'
                  : 'hover:bg-[#EEF4FF]'
            "
            @click="contextMenu.submenu = contextMenu.submenu === 'mode' ? null : 'mode'"
          >
            <span class="w-4 text-center">⚙</span>
            <span class="flex-1">切换注入方式</span>
            <span class="opacity-60">▸</span>
          </button>

          <div
            v-if="contextMenu.submenu === 'mode'"
            class="absolute top-0 w-[148px] rounded-lg border p-1 shadow-xl z-10"
            :class="[
              submenuFlipLeft ? 'right-full mr-1' : 'left-full ml-1',
              isDark
                ? 'bg-[#1B1F28] border-[#2B3240] text-[#ECF0F8]'
                : 'bg-[#FFFFFF] border-[#D8E0EC] text-[#171B24]',
            ]"
          >
            <button
              v-for="m in MODE_ORDER"
              :key="m"
              type="button"
              class="w-full h-8 px-2.5 rounded-md flex items-center justify-between text-left transition-colors cursor-pointer"
              :class="
                contextMenu.rule.mode === m
                  ? isDark
                    ? 'bg-[#1A2B4C] font-semibold'
                    : 'bg-[#E0ECFF] font-semibold'
                  : isDark
                    ? 'hover:bg-[#252D3D]'
                    : 'hover:bg-[#EEF4FF]'
              "
              @click="setRuleMode(contextMenu.rule, m); closeAllMenus()"
            >
              <span :style="{ color: getModePill(m).color }" class="font-bold">
                {{ getModePill(m).label }}
              </span>
              <span v-if="contextMenu.rule.mode === m" class="font-bold">✓</span>
            </button>
          </div>
        </div>

        <button
          type="button"
          class="w-full h-8 px-2.5 rounded-md flex items-center gap-2 text-left transition-colors cursor-pointer"
          :class="isDark ? 'hover:bg-[#252D3D]' : 'hover:bg-[#EEF4FF]'"
          @mouseenter="contextMenu.submenu = null"
          @click="emit('create-shortcut', contextMenu.rule); closeAllMenus()"
        >
          <span class="w-4 text-center">↗</span>
          <span>生成桌面直连图标</span>
        </button>

        <div
          class="my-1 border-t"
          :class="isDark ? 'border-[#252B38]' : 'border-[#E2E8F2]'"
        />

        <button
          type="button"
          class="w-full h-8 px-2.5 rounded-md flex items-center gap-2 text-left transition-colors cursor-pointer"
          :class="isDark ? 'hover:bg-[#252D3D]' : 'hover:bg-[#EEF4FF]'"
          @mouseenter="contextMenu.submenu = null"
          @click="emit('edit-rule', contextMenu.rule); closeAllMenus()"
        >
          <span class="w-4 text-center">✎</span>
          <span>编辑规则</span>
        </button>

        <button
          type="button"
          class="w-full h-8 px-2.5 rounded-md flex items-center gap-2 text-left transition-colors cursor-pointer"
          :class="
            isDark
              ? 'text-[#F87171] hover:bg-[#3B181B]'
              : 'text-[#DC2626] hover:bg-[#FEE2E2]'
          "
          @mouseenter="contextMenu.submenu = null"
          @click="emit('delete-rules', [contextMenu.rule.id]); closeAllMenus()"
        >
          <span class="w-4 text-center">✕</span>
          <span>删除规则</span>
        </button>
      </div>
    </Teleport>
  </section>
</template>

<style scoped>
@keyframes portSwitchFlash {
  0% {
    border-color: #3b82f6;
    box-shadow: 0 0 0 2px rgba(59, 130, 246, 0.35);
  }
  35% {
    border-color: #93c5fd;
    box-shadow: 0 0 0 3px rgba(96, 165, 250, 0.45);
  }
  70% {
    border-color: #3b82f6;
    box-shadow: 0 0 0 2px rgba(59, 130, 246, 0.25);
  }
  100% {
    box-shadow: 0 0 0 0 rgba(59, 130, 246, 0);
  }
}

.port-switch-flash {
  animation: portSwitchFlash 0.55s ease-out;
}
</style>
