export type AssetKind = "client_pack" | "server_pack" | "full_zip";
export type DownloadKind = "client" | "server" | "zip";

export interface AssetInfo {
  name: string;
  size: number;
  downloadUrl: string;
  kind: AssetKind;
}

export interface ReleaseInfo {
  tag: string;
  name: string;
  body: string | null;
  publishedAt: string | null;
  prerelease: boolean;
  assets: AssetInfo[];
}

export interface InstalledFile {
  name: string;
  size: number;
  kind: AssetKind | null;
}

export interface InstalledVersion {
  tag: string;
  dir: string;
  files: InstalledFile[];
  installedAt: number;
}

export interface InstalledServer {
  tag: string;
  dir: string;
  script: string | null;
  installedAt: number;
  propertiesPath?: string | null;
  onlineMode?: boolean | null;
  hasServerJar?: boolean;
  ramMb?: number;
}

export interface LibrarySnapshot {
  versions: InstalledVersion[];
  servers: InstalledServer[];
  running: string[];
}

export interface LauncherConfig {
  kind: string;
  exePath?: string | null;
}

export interface AppConfig {
  launcher: LauncherConfig | null;
  managerRepo?: string | null;
  javaPath?: string | null;
}

export interface DetectedJava {
  path: string;
  major: number;
}

export interface DetectedLauncher {
  kind: string;
  exePath: string;
}

export interface InstallResult {
  message: string;
}

export interface UpdateStatus {
  configured: boolean;
  currentVersion: string;
  latestVersion?: string | null;
  updateAvailable: boolean;
  url?: string | null;
}

export interface ProgressEvent {
  id: string;
  received: number;
  total: number;
}

export interface ExtractEvent {
  id: string;
  current: number;
  total: number;
}

export interface DoneEvent {
  id: string;
  ok: boolean;
  message?: string | null;
}

export interface LogEvent {
  tag: string;
  line: string;
}

export interface StatusEvent {
  tag: string;
  running: boolean;
  exitCode?: number | null;
}
