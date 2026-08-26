import { invoke } from "@tauri-apps/api/core";

import type {
  AppConfig,
  DetectedJava,
  DetectedLauncher,
  DownloadKind,
  InstallResult,
  LibrarySnapshot,
  ReleaseInfo,
  UpdateStatus,
} from "./types";

export const api = {
  fetchReleases: () => invoke<ReleaseInfo[]>("fetch_releases"),

  startDownload: (kind: DownloadKind, tag: string) =>
    invoke<string>("start_download", { kind, tag }),

  cancelDownload: (id: string) => invoke<void>("cancel_download", { id }),

  listLibrary: () => invoke<LibrarySnapshot>("list_library"),

  deleteVersion: (tag: string) => invoke<void>("delete_version", { tag }),

  deleteServer: (tag: string) => invoke<void>("delete_server", { tag }),

  revealPath: (path: string) => invoke<void>("reveal_path", { path }),

  getConfig: () => invoke<AppConfig>("get_config"),

  setLauncher: (kind: string, exePath: string | null) =>
    invoke<void>("set_launcher", { kind, exePath }),

  detectLaunchers: () => invoke<DetectedLauncher[]>("detect_launchers"),

  installToLauncher: (tag: string) =>
    invoke<InstallResult>("install_to_launcher", { tag }),

  startServer: (tag: string) => invoke<number>("start_server", { tag }),

  stopServer: (tag: string) => invoke<void>("stop_server", { tag }),

  sendServerCommand: (tag: string, command: string) =>
    invoke<void>("send_server_command", { tag, command }),

  setOnlineMode: (tag: string, enabled: boolean) =>
    invoke<boolean>("set_online_mode", { tag, enabled }),

  setServerRam: (tag: string, mb: number) =>
    invoke<number>("set_server_ram", { tag, mb }),

  setJavaPath: (path: string | null) => invoke<void>("set_java_path", { path }),

  detectJava: () => invoke<DetectedJava | null>("detect_java"),

  installFabricServer: (tag: string, mcVersion?: string) =>
    invoke<string>("install_fabric_server", { tag, mcVersion }),

  checkUpdates: () => invoke<UpdateStatus>("check_updates"),
};

export function formatBytes(bytes: number): string {
  if (!bytes || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** i;
  return `${value.toFixed(value >= 100 || i === 0 ? 0 : 1)} ${units[i]}`;
}

export function formatDate(iso: string | null | undefined): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return d.toLocaleDateString("en-US", {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

export function formatUnix(secs: number): string {
  if (!secs) return "";
  return formatDate(new Date(secs * 1000).toISOString());
}
