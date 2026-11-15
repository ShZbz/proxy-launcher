<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { api } from "./api";
import PortModal from "./components/PortModal.vue";
import PortSidebar from "./components/PortSidebar.vue";
import RuleModal from "./components/RuleModal.vue";
import RuleWorkspace from "./components/RuleWorkspace.vue";
import TerminalModal from "./components/TerminalModal.vue";
import {
  generateId,
  type Config,
  type Port,
  type ProbeResult,
  type ResolvedTarget,
  type Rule,
} from "./types";

interface RuleWorkspaceExposed {
  focusSearch?: () => void;
  getSelectedIds?: () => string[];
  clearSelection?: () => void;
}

interface ConfirmDialogState {
  visible: boolean;
  title: string;
  message: string;
  danger: boolean;
  confirmOnly: boolean;
  resolve: ((ok: boolean) => void) | null;
}

const THEME_STORAGE_KEY = "proxy-launcher-theme";

function detectInitialDark(): boolean {
  try {
    const saved = localStorage.getItem(THEME_STORAGE_KEY);
    if (saved === "dark") return true;
    if (saved === "light") return false;
  } catch {
    // ignore storage errors
  }
  return (
    typeof window !== "undefined" &&
    window.matchMedia?.("(prefers-color-scheme: dark)").matches
  );
}

const isDark = ref<boolean>(detectInitialDark());
const config = ref<Config>({
  version: 1,
  test_url: "http://cp.cloudflare.com/generate_204",
  ip_url: "https://api.ipify.org",
  show_exit_ip: true,
  ports: [],
  rules: [],
});

const probeResults = ref<Record<string, ProbeResult>>({});
const probingIds = ref<Record<string, boolean>>({});
const selectedPortId = ref<string | null>(null);

const statusText = ref<string>("就绪");
const statusTone = ref<"info" | "ok" | "error">("info");

const ruleWorkspaceRef = ref<RuleWorkspaceExposed | null>(null);

// Modals
const portModalVisible = ref(false);
const editingPort = ref<Port | null>(null);

const terminalModalVisible = ref(false);
const terminalModalPort = ref<Port | null>(null);

const ruleModalVisible = ref(false);
const editingRule = ref<Rule | null>(null);
const prefillTarget = ref<ResolvedTarget | null>(null);

const confirmState = ref<ConfirmDialogState>({
  visible: false,
  title: "",
  message: "",
  danger: false,
  confirmOnly: false,
  resolve: null,
});

const defaultPortId = computed<string>(() => {
  if (
    selectedPortId.value &&
    config.value.ports.some((p) => p.id === selectedPortId.value)
  ) {
    return selectedPortId.value;
  }
  return config.value.ports[0]?.id ?? "";
});

const anyModalOpen = computed<boolean>(
  () =>
    portModalVisible.value ||
    terminalModalVisible.value ||
    ruleModalVisible.value ||
    confirmState.value.visible
);

function applyThemeClass(dark: boolean): void {
  if (typeof document !== "undefined") {
    document.documentElement.classList.toggle("dark", dark);
  }
}

function setStatus(text: string, tone: "info" | "ok" | "error" = "info"): void {
  statusText.value = text;
  statusTone.value = tone;
}

function toggleTheme(): void {
  isDark.value = !isDark.value;
  applyThemeClass(isDark.value);
  try {
    localStorage.setItem(THEME_STORAGE_KEY, isDark.value ? "dark" : "light");
  } catch {
    // ignore storage errors
  }
  setStatus(`已切换为${isDark.value ? "深色" : "浅色"}主题`);
}

function formatPortLabel(port: Port): string {
  return port.alias?.trim() || `${port.host}:${port.port}`;
}

function extractBasename(filePath: string): string {
  const parts = filePath.split(/[/\\]/);
  return parts[parts.length - 1] || filePath;
}

function showConfirm(
  title: string,
  message: string,
  danger = false
): Promise<boolean> {
  return new Promise((resolve) => {
    confirmState.value = {
      visible: true,
      title,
      message,
      danger,
      confirmOnly: false,
      resolve,
    };
  });
}

function showInfo(title: string, message: string): Promise<boolean> {
  return new Promise((resolve) => {
    confirmState.value = {
      visible: true,
      title,
      message,
      danger: false,
      confirmOnly: true,
      resolve,
    };
  });
}

function resolveConfirm(accepted: boolean): void {
  const resolver = confirmState.value.resolve;
  confirmState.value = {
    visible: false,
    title: "",
    message: "",
    danger: false,
    confirmOnly: false,
    resolve: null,
  };
  resolver?.(accepted);
}

async function persistConfig(): Promise<boolean> {
  try {
    await api.saveConfig(config.value);
    return true;
  } catch (err) {
    setStatus(`保存配置失败：${String(err)}`, "error");
    return false;
  }
}

async function loadInitialConfig(): Promise<void> {
  try {
    const loaded = await api.getConfig();
    config.value = loaded;
    setStatus(
      `就绪 · ${loaded.ports.length} 个端口 · ${loaded.rules.length} 条规则 · 支持一键打开代理终端与创建直连快捷方式`
    );
  } catch (err) {
    setStatus(`读取配置失败（预览模式或后端未连接）：${String(err)}`, "error");
  }
}

// ------------------------------------------------------------------ Ports

function handleSelectPort(portId: string | null): void {
  const next = selectedPortId.value === portId ? null : portId;
  selectedPortId.value = next;
  if (next) {
    const port = config.value.ports.find((p) => p.id === next);
    if (port) {
      const count = config.value.rules.filter((r) => r.port_id === next).length;
      setStatus(`已按端口「${formatPortLabel(port)}」筛选 · 显示 ${count} 条规则`);
      return;
    }
  }
  setStatus(`已显示全部 ${config.value.rules.length} 条规则`);
}

function openAddPortModal(): void {
  editingPort.value = null;
  portModalVisible.value = true;
}

function openEditPortModal(port: Port): void {
  editingPort.value = { ...port };
  portModalVisible.value = true;
}

async function handleSavePort(portData: {
  id?: string;
  alias: string;
  host: string;
  port: number;
}): Promise<void> {
  const host = portData.host.trim() || "127.0.0.1";
  const alias = portData.alias.trim() || `port ${portData.port}`;
  const duplicate = config.value.ports.find(
    (p) => p.host === host && p.port === portData.port && p.id !== portData.id
  );
  if (duplicate) {
    setStatus(`${host}:${portData.port} 已存在`, "error");
    return;
  }

  let savedPort: Port;
  if (portData.id) {
    const idx = config.value.ports.findIndex((p) => p.id === portData.id);
    savedPort = { id: portData.id, alias, host, port: portData.port };
    if (idx >= 0) {
      config.value.ports.splice(idx, 1, savedPort);
    } else {
      config.value.ports.push(savedPort);
    }
  } else {
    savedPort = { id: generateId(), alias, host, port: portData.port };
    config.value.ports.push(savedPort);
  }

  portModalVisible.value = false;
  if (await persistConfig()) {
    setStatus(`已保存端口「${formatPortLabel(savedPort)}」`, "ok");
  }
}

async function handleDeletePort(port: Port): Promise<void> {
  const users = config.value.rules.filter((r) => r.port_id === port.id);
  const label = formatPortLabel(port);
  if (users.length > 0) {
    const ok = await showConfirm(
      "删除端口",
      `${users.length} 条规则正在使用「${label}」，删除后它们将变成未绑定。确定删除？`,
      true
    );
    if (!ok) return;
  }

  config.value.ports = config.value.ports.filter((p) => p.id !== port.id);
  const nextProbes = { ...probeResults.value };
  delete nextProbes[port.id];
  probeResults.value = nextProbes;

  if (selectedPortId.value === port.id) {
    selectedPortId.value = null;
  }

  if (await persistConfig()) {
    setStatus(`已删除端口「${label}」`);
  }
}

async function handleProbePort(port: Port): Promise<void> {
  probingIds.value = { ...probingIds.value, [port.id]: true };
  try {
    const res = await api.probePort(port);
    probeResults.value = { ...probeResults.value, [port.id]: res };
    const label = formatPortLabel(port);
    if (res.ok) {
      const lat = res.latency_ms !== null ? ` · ${res.latency_ms}ms` : "";
      setStatus(`端口「${label}」正常${lat}`, "ok");
    } else {
      setStatus(`端口「${label}」异常：${res.reason_text || res.detail}`, "error");
    }
  } catch (err) {
    setStatus(`测试端口失败：${String(err)}`, "error");
  } finally {
    const next = { ...probingIds.value };
    delete next[port.id];
    probingIds.value = next;
  }
}

async function handleProbeAll(): Promise<void> {
  const ports = config.value.ports;
  if (ports.length === 0) {
    setStatus("没有端口可测试");
    return;
  }

  const busyMap: Record<string, boolean> = {};
  for (const p of ports) {
    busyMap[p.id] = true;
  }
  probingIds.value = busyMap;
  setStatus(`正在测试 ${ports.length} 个端口…`);

  try {
    const results = await api.probeAllPorts();
    probeResults.value = { ...probeResults.value, ...results };
    const okCount = Object.values(results).filter((r) => r.ok).length;
    setStatus(
      `${ports.length} 个端口 · ${okCount} 正常 · ${ports.length - okCount} 异常`,
      okCount === ports.length ? "ok" : "info"
    );
  } catch (err) {
    setStatus(`批量测试失败：${String(err)}`, "error");
  } finally {
    probingIds.value = {};
  }
}

// ------------------------------------------------------------------ Terminal

function resolveTargetPort(port: Port | null): Port | null {
  if (port) return port;
  if (selectedPortId.value) {
    const selected = config.value.ports.find(
      (p) => p.id === selectedPortId.value
    );
    if (selected) return selected;
  }
  return config.value.ports[0] ?? null;
}

async function handleOpenTerminal(
  port: Port | null,
  shell: "powershell" | "cmd" = "powershell"
): Promise<void> {
  const target = resolveTargetPort(port);
  if (!target) {
    await showInfo("没有代理端口", "请先在左侧添加一个代理端口，再打开代理终端。");
    return;
  }

  try {
    await api.launchTerminal(target.id, shell);
    const label = formatPortLabel(target);
    setStatus(
      `已打开代理终端 (${label} -> http://${target.host}:${target.port})`,
      "ok"
    );
  } catch (err) {
    setStatus(
      `直接打开终端受限 (${String(err)})，已为您弹出终端代理注入代码`,
      "error"
    );
    handleOpenTerminalSnippet(target);
  }
}

async function handleLaunchShellById(
  portId: string,
  shell: "powershell" | "cmd"
): Promise<void> {
  const port = config.value.ports.find((p) => p.id === portId) ?? null;
  await handleOpenTerminal(port, shell);
}

async function handleOpenTerminalSnippet(port: Port | null): Promise<void> {
  const target = resolveTargetPort(port);
  if (!target) {
    await showInfo("没有代理端口", "请先在左侧添加一个代理端口。");
    return;
  }
  terminalModalPort.value = target;
  terminalModalVisible.value = true;
}

// ------------------------------------------------------------------ Rules & DnD

function openAddRuleModal(target: ResolvedTarget | null = null): void {
  editingRule.value = null;
  prefillTarget.value = target;
  ruleModalVisible.value = true;
}

async function handleAddRuleFlow(): Promise<void> {
  if (config.value.ports.length === 0) {
    await showInfo("先添加端口", "还没有任何端口，请先在左侧添加一个代理端口。");
    return;
  }
  openAddRuleModal(null);
}

function openEditRuleModal(rule: Rule): void {
  editingRule.value = { ...rule };
  prefillTarget.value = null;
  ruleModalVisible.value = true;
}

function closeRuleModal(): void {
  ruleModalVisible.value = false;
  editingRule.value = null;
  prefillTarget.value = null;
}

async function handleSaveRule(
  rule: Rule,
  createShortcutAfter: boolean
): Promise<void> {
  const idx = config.value.rules.findIndex((r) => r.id === rule.id);
  if (idx >= 0) {
    config.value.rules.splice(idx, 1, rule);
  } else {
    config.value.rules.push(rule);
  }
  closeRuleModal();

  if (await persistConfig()) {
    setStatus(`已保存「${rule.name || extractBasename(rule.exe)}」`, "ok");
    if (createShortcutAfter) {
      await handleCreateShortcut(rule);
    }
  }
}

async function handleUpdateRule(updated: Rule): Promise<void> {
  const idx = config.value.rules.findIndex((r) => r.id === updated.id);
  if (idx < 0) return;

  const prev = config.value.rules[idx];
  config.value.rules.splice(idx, 1, updated);

  if (await persistConfig()) {
    if (prev.port_id !== updated.port_id) {
      const port = config.value.ports.find((p) => p.id === updated.port_id);
      const portLabel = port ? formatPortLabel(port) : "未绑定";
      setStatus(
        `已将「${updated.name}」的代理端口切换为「${portLabel}」`,
        "ok"
      );
    } else if (prev.mode !== updated.mode) {
      setStatus(`已将「${updated.name}」注入模式切换为 ${updated.mode}`);
    }
  }
}

async function handleBatchRebindPort(
  ruleIds: string[],
  portId: string
): Promise<void> {
  const port = config.value.ports.find((p) => p.id === portId);
  if (!port) return;
  const idSet = new Set(ruleIds);
  let count = 0;
  config.value.rules = config.value.rules.map((r) => {
    if (idSet.has(r.id)) {
      count++;
      return { ...r, port_id: portId };
    }
    return r;
  });
  if (count === 0) return;

  if (await persistConfig()) {
    setStatus(
      `已将 ${count} 条规则的代理端口切换为「${formatPortLabel(port)}」`,
      "ok"
    );
  }
}

async function handleDeleteRules(ruleIds: string[]): Promise<void> {
  if (ruleIds.length === 0) return;
  const targets = config.value.rules.filter((r) => ruleIds.includes(r.id));
  if (targets.length === 0) return;

  const prompt =
    targets.length === 1
      ? `确定删除「${targets[0].name || targets[0].exe}」？`
      : `确定删除选中的 ${targets.length} 条规则？`;
  const ok = await showConfirm("删除规则", prompt, true);
  if (!ok) return;

  const idSet = new Set(ruleIds);
  config.value.rules = config.value.rules.filter((r) => !idSet.has(r.id));
  ruleWorkspaceRef.value?.clearSelection?.();

  if (await persistConfig()) {
    setStatus(
      targets.length === 1 ? "已删除规则" : `已删除 ${targets.length} 条规则`
    );
  }
}

async function handleLaunchRule(rule: Rule): Promise<void> {
  if (!rule.enabled) {
    setStatus(`「${rule.name}」已禁用，请先启用`, "error");
    return;
  }
  const port = config.value.ports.find((p) => p.id === rule.port_id);
  if (!port) {
    setStatus(`「${rule.name}」没有绑定端口`, "error");
    return;
  }

  try {
    const res = await api.launchRule(rule.id);
    if (res.ok) {
      setStatus(`${rule.name} 已启动 · pid ${res.pid}`, "ok");
    } else {
      setStatus(`${rule.name} 启动失败：${res.message}`, "error");
    }
  } catch (err) {
    setStatus(`${rule.name} 启动失败：${String(err)}`, "error");
  }
}

async function handleBatchLaunch(ruleIds: string[]): Promise<void> {
  const targets = config.value.rules.filter(
    (r) => ruleIds.includes(r.id) && r.enabled
  );
  if (targets.length === 0) {
    setStatus("没有选中的启用规则");
    return;
  }

  let okCount = 0;
  let failCount = 0;
  for (const rule of targets) {
    const port = config.value.ports.find((p) => p.id === rule.port_id);
    if (!port) {
      failCount++;
      continue;
    }
    try {
      const res = await api.launchRule(rule.id);
      if (res.ok) okCount++;
      else failCount++;
    } catch {
      failCount++;
    }
  }

  if (failCount === 0) {
    setStatus(`已批量启动 ${okCount} 个应用`, "ok");
  } else {
    setStatus(`启动完成：${okCount} 成功 · ${failCount} 失败`, "error");
  }
}

async function handleCreateShortcut(rule: Rule): Promise<void> {
  const port = config.value.ports.find((p) => p.id === rule.port_id);
  if (!port) {
    setStatus(
      `「${rule.name}」尚未绑定端口，请先绑定端口再生成快捷方式`,
      "error"
    );
    return;
  }

  try {
    const lnkPath = await api.createShortcut(rule.id);
    setStatus(
      `已在桌面生成直连图标：${extractBasename(lnkPath)}（双击无需打开启动器直接走代理启动）`,
      "ok"
    );
  } catch (err) {
    setStatus(`创建快捷方式失败：${String(err)}`, "error");
  }
}

async function handleBatchShortcut(ruleIds: string[]): Promise<void> {
  const targets = config.value.rules.filter((r) => ruleIds.includes(r.id));
  if (targets.length === 0) {
    setStatus("请先勾选要生成桌面直连图标的规则");
    return;
  }

  let created = 0;
  for (const rule of targets) {
    if (!config.value.ports.some((p) => p.id === rule.port_id)) continue;
    try {
      await api.createShortcut(rule.id);
      created++;
    } catch {
      // continue remaining
    }
  }

  if (created > 0) {
    setStatus(`已为 ${created} 条规则在桌面生成直连快捷方式`, "ok");
  } else {
    setStatus("生成桌面快捷方式失败（请检查规则是否绑定有效端口）", "error");
  }
}

async function handleFilesDropped(paths: string[]): Promise<void> {
  for (const filePath of paths) {
    let target: ResolvedTarget;
    try {
      target = await api.resolveDroppedPath(filePath);
    } catch (err) {
      setStatus(`无法添加：${String(err)}`, "error");
      continue;
    }

    const existing = config.value.rules.find(
      (r) => r.exe.trim().toLowerCase() === target.exe.trim().toLowerCase()
    );
    if (existing) {
      const ok = await showConfirm(
        "重复的目标",
        `已有规则「${existing.name}」指向这个程序，仍要新建吗？`
      );
      if (!ok) continue;
    }

    if (config.value.ports.length === 0) {
      await showInfo("先添加端口", "还没有任何端口，请先在左侧添加一个代理端口。");
      return;
    }

    openAddRuleModal(target);
    break;
  }
}

// ------------------------------------------------------------------ Shortcuts

function isEditableElement(el: EventTarget | null): boolean {
  if (!(el instanceof HTMLElement)) return false;
  const tag = el.tagName;
  return (
    tag === "INPUT" ||
    tag === "TEXTAREA" ||
    tag === "SELECT" ||
    el.isContentEditable
  );
}

function handleKeyDown(e: KeyboardEvent): void {
  if (confirmState.value.visible) {
    if (e.key === "Escape") {
      e.preventDefault();
      resolveConfirm(false);
    } else if (e.key === "Enter") {
      e.preventDefault();
      resolveConfirm(true);
    }
    return;
  }

  if (anyModalOpen.value) return;

  const ctrl = e.ctrlKey || e.metaKey;
  const key = e.key.toLowerCase();

  if (ctrl && e.shiftKey && key === "t") {
    e.preventDefault();
    void handleOpenTerminal(null, "powershell");
    return;
  }

  if (ctrl && !e.shiftKey && key === "n") {
    e.preventDefault();
    void handleAddRuleFlow();
    return;
  }

  if (ctrl && !e.shiftKey && key === "t") {
    e.preventDefault();
    void handleProbeAll();
    return;
  }

  if (ctrl && !e.shiftKey && key === "f") {
    e.preventDefault();
    ruleWorkspaceRef.value?.focusSearch?.();
    return;
  }

  if (ctrl && e.key === "Enter") {
    e.preventDefault();
    const ids = ruleWorkspaceRef.value?.getSelectedIds?.() ?? [];
    void handleBatchLaunch(ids);
    return;
  }

  if (e.key === "Delete" && !isEditableElement(e.target)) {
    const ids = ruleWorkspaceRef.value?.getSelectedIds?.() ?? [];
    if (ids.length > 0) {
      e.preventDefault();
      void handleDeleteRules(ids);
    }
  }
}

let unlistenDrop: (() => void) | null = null;

onMounted(async () => {
  applyThemeClass(isDark.value);
  window.addEventListener("keydown", handleKeyDown);
  unlistenDrop = await api.onFileDrop((paths) => {
    void handleFilesDropped(paths);
  });
  await loadInitialConfig();
});

onUnmounted(() => {
  window.removeEventListener("keydown", handleKeyDown);
  unlistenDrop?.();
});
</script>

<template>
  <div
    class="h-screen w-screen flex flex-col overflow-hidden"
    :class="{ dark: isDark }"
    :style="{ backgroundColor: 'var(--bg)', color: 'var(--text)' }"
  >
    <!-- Main Two-Pane Workspace -->
    <div class="flex-1 flex min-h-0 overflow-hidden">
      <div class="w-[340px] shrink-0 h-full border-r" :style="{ borderColor: 'var(--border)' }">
        <PortSidebar
          :ports="config.ports"
          :rules="config.rules"
          :probe-results="probeResults"
          :probing-ids="probingIds"
          :selected-port-id="selectedPortId"
          :is-dark="isDark"
          @select-port="handleSelectPort"
          @probe-port="handleProbePort"
          @probe-all="handleProbeAll"
          @add-port="openAddPortModal"
          @edit-port="openEditPortModal"
          @delete-port="handleDeletePort"
          @open-terminal="handleOpenTerminal"
          @open-terminal-snippet="handleOpenTerminalSnippet"
          @toggle-theme="toggleTheme"
        />
      </div>

      <div class="flex-1 min-w-0 h-full">
        <RuleWorkspace
          ref="ruleWorkspaceRef"
          :rules="config.rules"
          :ports="config.ports"
          :probe-results="probeResults"
          :selected-port-id="selectedPortId"
          :is-dark="isDark"
          @clear-port-filter="handleSelectPort(null)"
          @add-rule="handleAddRuleFlow"
          @edit-rule="openEditRuleModal"
          @delete-rules="handleDeleteRules"
          @update-rule="handleUpdateRule"
          @batch-rebind-port="handleBatchRebindPort"
          @launch-rule="handleLaunchRule"
          @batch-launch="handleBatchLaunch"
          @create-shortcut="handleCreateShortcut"
          @batch-shortcut="handleBatchShortcut"
          @files-dropped="handleFilesDropped"
        />
      </div>
    </div>

    <!-- Bottom Status Bar (32px) -->
    <footer
      class="h-8 shrink-0 px-3 flex items-center justify-between gap-3 border-t text-xs"
      :style="{
        backgroundColor: 'var(--sidebar)',
        borderColor: 'var(--border)',
      }"
    >
      <div class="flex items-center gap-2 min-w-0">
        <span
          class="inline-block w-2 h-2 rounded-full shrink-0"
          :style="{
            backgroundColor:
              statusTone === 'ok'
                ? isDark
                  ? '#4ADE80'
                  : '#15803D'
                : statusTone === 'error'
                ? 'var(--danger)'
                : 'var(--text-dim)',
          }"
        />
        <span
          class="truncate"
          :style="{
            color:
              statusTone === 'ok'
                ? isDark
                  ? '#4ADE80'
                  : '#15803D'
                : statusTone === 'error'
                ? 'var(--danger)'
                : 'var(--text-dim)',
          }"
          :title="statusText"
        >
          {{ statusText }}
        </span>
      </div>

      <div
        class="hidden sm:flex items-center gap-2 shrink-0 text-[11px] font-mono-code"
        :style="{ color: 'var(--text-dim)' }"
      >
        <span>Ctrl+N 新增</span>
        <span>·</span>
        <span>Ctrl+T 测速</span>
        <span>·</span>
        <span>Ctrl+Shift+T 终端</span>
        <span>·</span>
        <span>Ctrl+F 搜索</span>
      </div>
    </footer>

    <!-- Modals -->
    <PortModal
      :visible="portModalVisible"
      :editing-port="editingPort"
      :is-dark="isDark"
      @close="portModalVisible = false"
      @save="handleSavePort"
    />

    <TerminalModal
      :visible="terminalModalVisible"
      :port="terminalModalPort"
      :is-dark="isDark"
      @close="terminalModalVisible = false"
      @launch-shell="handleLaunchShellById"
      @notify="(msg) => setStatus(msg, 'ok')"
    />

    <RuleModal
      :visible="ruleModalVisible"
      :editing-rule="editingRule"
      :prefill-target="prefillTarget"
      :ports="config.ports"
      :default-port-id="defaultPortId"
      :is-dark="isDark"
      @close="closeRuleModal"
      @save="handleSaveRule"
    />

    <!-- In-App Confirm / Info Modal -->
    <div
      v-if="confirmState.visible"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/45 backdrop-blur-[1px] p-4"
      @click.self="resolveConfirm(false)"
    >
      <div
        class="w-full max-w-[400px] rounded-xl border p-5 shadow-xl"
        :style="{
          backgroundColor: 'var(--surface)',
          borderColor: 'var(--border)',
          color: 'var(--text)',
        }"
      >
        <h3 class="text-base font-semibold mb-2">{{ confirmState.title }}</h3>
        <p class="text-sm leading-relaxed mb-5" :style="{ color: 'var(--text-dim)' }">
          {{ confirmState.message }}
        </p>
        <div class="flex justify-end gap-2">
          <button
            v-if="!confirmState.confirmOnly"
            type="button"
            class="px-4 py-1.5 rounded-lg text-sm border cursor-pointer transition-colors"
            :style="{
              borderColor: 'var(--border)',
              color: 'var(--text-dim)',
              backgroundColor: 'transparent',
            }"
            @click="resolveConfirm(false)"
          >
            取消
          </button>
          <button
            type="button"
            class="px-4 py-1.5 rounded-lg text-sm font-medium text-white cursor-pointer transition-opacity hover:opacity-90"
            :style="{
              backgroundColor: confirmState.danger
                ? 'var(--danger)'
                : 'var(--accent)',
            }"
            @click="resolveConfirm(true)"
          >
            {{ confirmState.confirmOnly ? "知道了" : "确定" }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
