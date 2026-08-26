import { listen } from "@tauri-apps/api/event";

import type { DoneEvent, ExtractEvent, ProgressEvent } from "./types";

export type DownloadPhase = "downloading" | "extracting" | "done";

export interface DownloadState {
  id: string;
  received: number;
  total: number;
  phase: DownloadPhase;
}

type Listener = (id: string, result: { ok: boolean; message?: string | null }) => void;

class DownloadsStore {
  map = $state<Record<string, DownloadState>>({});
  private doneListeners = new Set<Listener>();

  async init() {
    await listen<ProgressEvent>("download-progress", (e) => {
      this.update(e.payload.id, {
        received: e.payload.received,
        total: e.payload.total,
        phase: "downloading",
      });
    });

    await listen<ExtractEvent>("extract-progress", (e) => {
      this.update(e.payload.id, {
        received: e.payload.current,
        total: e.payload.total,
        phase: "extracting",
      });
    });

    await listen<DoneEvent>("download-done", (e) => {
      const { id, ok } = e.payload;
      if (!ok) {
        delete this.map[id];
      } else if (this.map[id]) {
        this.map[id].phase = "done";
      }
      for (const fn of this.doneListeners) fn(id, e.payload);
    });
  }

  private update(id: string, patch: Partial<DownloadState>) {
    const existing = this.map[id];
    this.map[id] = {
      id,
      received: patch.received ?? existing?.received ?? 0,
      total: patch.total ?? existing?.total ?? 0,
      phase: patch.phase ?? existing?.phase ?? "downloading",
    };
  }

  active(id: string): boolean {
    const s = this.map[id];
    return !!s && s.phase !== "done";
  }

  state(id: string): DownloadState | undefined {
    return this.map[id];
  }

  percent(id: string): number {
    const s = this.map[id];
    if (!s || !s.total) return 0;
    return Math.min(100, Math.round((s.received / s.total) * 100));
  }

  activeCount(): number {
    return Object.values(this.map).filter((d) => d.phase !== "done").length;
  }

  onDone(fn: Listener): () => void {
    this.doneListeners.add(fn);
    return () => this.doneListeners.delete(fn);
  }
}

export const downloads = new DownloadsStore();
