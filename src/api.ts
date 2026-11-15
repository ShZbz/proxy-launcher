import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type {
  Config,
  LaunchResult,
  Port,
  ProbeResult,
  ResolvedTarget,
} from "./types";

function isTauriEnv(): boolean {
  return (
    typeof window !== "undefined" &&
    ("__TAURI_INTERNALS__" in window || "__TAURI__" in window)
  );
}

export const api = {
  getConfig(): Promise<Config> {
    return invoke<Config>("get_config");
  },

  saveConfig(config: Config): Promise<void> {
    return invoke<void>("save_config", { config });
  },

  probePort(port: Port): Promise<ProbeResult> {
    return invoke<ProbeResult>("probe_port", { port });
  },

  probeAllPorts(): Promise<Record<string, ProbeResult>> {
    return invoke<Record<string, ProbeResult>>("probe_all_ports");
  },

  launchRule(ruleId: string): Promise<LaunchResult> {
    return invoke<LaunchResult>("launch_rule", { ruleId });
  },

  createShortcut(ruleId: string): Promise<string> {
    return invoke<string>("create_shortcut", { ruleId });
  },

  launchTerminal(
    portId: string,
    shell: "powershell" | "pwsh" | "cmd"
  ): Promise<void> {
    return invoke<void>("launch_terminal", { portId, shell });
  },

  pickExecutableDialog(): Promise<string | null> {
    return invoke<string | null>("pick_executable_dialog");
  },

  resolveDroppedPath(path: string): Promise<ResolvedTarget> {
    return invoke<ResolvedTarget>("resolve_dropped_path", { path });
  },

  async onFileDrop(callback: (paths: string[]) => void): Promise<() => void> {
    if (!isTauriEnv()) {
      return () => {};
    }
    try {
      const webview = getCurrentWebview();
      const unlisten = await webview.onDragDropEvent((event) => {
        if (event.payload.type === "drop" && event.payload.paths.length > 0) {
          callback(event.payload.paths);
        }
      });
      return unlisten;
    } catch {
      return () => {};
    }
  },
};
