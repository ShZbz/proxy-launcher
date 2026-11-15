export type InjectionMode = "env" | "flag" | "both";

export interface Port {
  id: string;
  alias: string;
  host: string;
  port: number;
}

export interface Rule {
  id: string;
  name: string;
  exe: string;
  port_id: string;
  args: string;
  cwd: string;
  mode: InjectionMode;
  flag_template: string;
  enabled: boolean;
  source: string;
}

export interface Config {
  version: number;
  test_url: string;
  ip_url: string;
  show_exit_ip: boolean;
  ports: Port[];
  rules: Rule[];
}

export interface ProbeResult {
  host: string;
  port: number;
  ok: boolean;
  reason: string;
  reason_text: string;
  detail: string;
  latency_ms: number | null;
  http_code: number | null;
  exit_ip: string | null;
}

export interface LaunchResult {
  pid: number;
  exe: string;
  ok: boolean;
  message: string;
}

export interface ResolvedTarget {
  exe: string;
  name: string;
  args: string;
  cwd: string;
  icon: string;
  needs_elevation: boolean;
  kind: string;
  parse_note: string;
  source: string;
}

type ThemePair = readonly [light: string, dark: string];

const pick = (pair: ThemePair, isDark: boolean): string =>
  isDark ? pair[1] : pair[0];

export const THEME_TOKENS = {
  bg: ["#F3F5F9", "#12151B"] as ThemePair,
  surface: ["#FFFFFF", "#1B1F28"] as ThemePair,
  surface2: ["#EBF0F7", "#242A36"] as ThemePair,
  surfaceHover: ["#F6F9FD", "#212734"] as ThemePair,
  sidebar: ["#E7ECF4", "#0E1116"] as ThemePair,
  border: ["#D8E0EC", "#2B3240"] as ThemePair,
  cardBorder: ["#E2E8F2", "#252B38"] as ThemePair,
  cardBorderHover: ["#94BCEB", "#456B9E"] as ThemePair,
  cardSelectedBg: ["#E8F1FF", "#16243A"] as ThemePair,
  cardSelectedBorder: ["#2563EB", "#60A5FA"] as ThemePair,
  portBadgeBg: ["#EBF0F7", "#242A36"] as ThemePair,
  portBadgeHover: ["#DCE7F7", "#2E3748"] as ThemePair,
  popupBg: ["#FFFFFF", "#1B1F28"] as ThemePair,
  popupHover: ["#EEF4FF", "#252D3D"] as ThemePair,
  popupActiveBg: ["#E0ECFF", "#1A2B4C"] as ThemePair,
  filterChipBg: ["#DBEAFE", "#1A2B4C"] as ThemePair,
  filterChipHover: ["#BFDBFE", "#243B66"] as ThemePair,
  filterChipFg: ["#1D4ED8", "#93C5FD"] as ThemePair,
  text: ["#171B24", "#ECF0F8"] as ThemePair,
  textDim: ["#5C6678", "#8B95A9"] as ThemePair,
  textOnAccent: "#FFFFFF",
  accent: ["#3B82F6", "#60A5FA"] as ThemePair,
  accentHi: ["#2563EB", "#93C5FD"] as ThemePair,
  accentSoft: ["#E0ECFF", "#1A2B4C"] as ThemePair,
  termAccent: ["#0D9488", "#2DD4BF"] as ThemePair,
  termHover: ["#0F766E", "#5EEAD4"] as ThemePair,
  termSoft: ["#CCFBF1", "#112E2B"] as ThemePair,
  shortcutAccent: ["#4F46E5", "#818CF8"] as ThemePair,
  shortcutSoft: ["#E0E7FF", "#1E1B4B"] as ThemePair,
  codeBg: ["#1E2430", "#0B0E14"] as ThemePair,
  codeFg: ["#E2E8F0", "#E2E8F0"] as ThemePair,
  codeBorder: ["#333D4F", "#232A38"] as ThemePair,
  danger: ["#DC2626", "#F87171"] as ThemePair,
  dangerSoft: ["#FEE2E2", "#3B181B"] as ThemePair,
} as const;

const REASON_STYLE_MAP: Record<
  string,
  { label: string; fg: ThemePair; bg: ThemePair }
> = {
  OK: {
    label: "正常",
    fg: ["#15803D", "#4ADE80"],
    bg: ["#DCFCE7", "#112E1F"],
  },
  PORT_CLOSED: {
    label: "端口没开",
    fg: ["#B91C1C", "#F87171"],
    bg: ["#FEE2E2", "#3A181B"],
  },
  PROXY_REFUSED: {
    label: "代理拒绝",
    fg: ["#991B1B", "#FCA5A5"],
    bg: ["#FEE2E2", "#3A1B19"],
  },
  PROXY_UPSTREAM_FAIL: {
    label: "上游失败",
    fg: ["#B45309", "#FBBF24"],
    bg: ["#FEF3C7", "#3A2810"],
  },
  HTTP_FAILED: {
    label: "节点异常",
    fg: ["#B45309", "#FBBF24"],
    bg: ["#FEF3C7", "#3A2810"],
  },
  TARGET_UNREACHABLE: {
    label: "目标不通",
    fg: ["#854D0E", "#FDE047"],
    bg: ["#FEF9C3", "#362F0E"],
  },
  TIMEOUT: {
    label: "超时",
    fg: ["#9A3412", "#FDBA74"],
    bg: ["#FFEDD5", "#3A2510"],
  },
  BAD_RESPONSE: {
    label: "响应异常",
    fg: ["#6D28D9", "#C4B5FD"],
    bg: ["#EDE9FE", "#2B1D42"],
  },
  NOT_A_PROXY: {
    label: "非代理端口",
    fg: ["#475569", "#94A3B8"],
    bg: ["#E2E8F0", "#262B36"],
  },
  NETWORK_ERROR: {
    label: "网络错误",
    fg: ["#9D174D", "#F472B6"],
    bg: ["#FCE7F3", "#3A1A28"],
  },
  UNTESTED: {
    label: "未测试",
    fg: ["#5C6678", "#8B95A9"],
    bg: ["#EBF0F7", "#242A36"],
  },
  TESTING: {
    label: "测试中…",
    fg: ["#2563EB", "#60A5FA"],
    bg: ["#DBEAFE", "#1B2A4A"],
  },
};

export function getReasonStyle(
  reason: string,
  isDark: boolean
): { label: string; fg: string; bg: string } {
  const entry = REASON_STYLE_MAP[reason] ?? REASON_STYLE_MAP.UNTESTED;
  return {
    label: entry.label,
    fg: pick(entry.fg, isDark),
    bg: pick(entry.bg, isDark),
  };
}

const LAT_OK: ThemePair = ["#15803D", "#4ADE80"];
const LAT_WARN: ThemePair = ["#B45309", "#FBBF24"];
const LAT_BAD: ThemePair = THEME_TOKENS.danger;

export function getLatencyColor(ms: number | null, isDark: boolean): string {
  if (ms === null || ms === undefined) {
    return pick(THEME_TOKENS.textDim, isDark);
  }
  if (ms < 300) return pick(LAT_OK, isDark);
  if (ms < 1000) return pick(LAT_WARN, isDark);
  return pick(LAT_BAD, isDark);
}

const MODE_STYLE_MAP: Record<string, { label: string; color: ThemePair }> = {
  env: { label: "env", color: ["#2563EB", "#7FB0FF"] },
  flag: { label: "flag", color: ["#0D9488", "#5FE0CF"] },
  both: { label: "both", color: ["#7C3AED", "#C4B5FD"] },
};

export function getModeStyle(
  mode: string,
  isDark: boolean
): { label: string; color: string } {
  const normalized = (mode || "both").trim().toLowerCase();
  const entry = MODE_STYLE_MAP[normalized] ?? MODE_STYLE_MAP.both;
  return {
    label: entry.label,
    color: pick(entry.color, isDark),
  };
}

const AVATAR_PALETTE = [
  "#8C7A6B",
  "#6F7F8C",
  "#7A6E8C",
  "#8C6F7E",
  "#6E8C7F",
  "#8C8A6F",
  "#6F7A8C",
  "#7F8C8A",
] as const;

export function getAvatarColor(name: string): string {
  const input = name || "?";
  let hash = 2166136261;
  for (let i = 0; i < input.length; i++) {
    hash ^= input.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  const idx = (hash >>> 0) % AVATAR_PALETTE.length;
  return AVATAR_PALETTE[idx];
}

export function getAvatarLetter(name: string): string {
  const trimmed = (name || "").trim();
  if (!trimmed) return "?";
  return trimmed.charAt(0).toUpperCase();
}

export function generateId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.getRandomValues === "function") {
    const bytes = new Uint8Array(6);
    crypto.getRandomValues(bytes);
    return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
  }
  let out = "";
  for (let i = 0; i < 12; i++) {
    out += Math.floor(Math.random() * 16).toString(16);
  }
  return out;
}
