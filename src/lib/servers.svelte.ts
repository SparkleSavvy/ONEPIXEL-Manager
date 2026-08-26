import { listen } from "@tauri-apps/api/event";

import type { LogEvent, StatusEvent } from "./types";

type Listener = () => void;

/**
 * Global, tab-independent server state: console logs and running flags.
 * Lives outside components so switching pages never drops output.
 */
class ServersStore {
  logs = $state<Record<string, string[]>>({});
  running = $state<Record<string, boolean>>({});

  private listeners = new Set<Listener>();
  private started = false;

  async init() {
    if (this.started) return;
    this.started = true;

    await listen<LogEvent>("server-log", (e) => {
      this.pushLog(e.payload.tag, e.payload.line);
    });

    await listen<StatusEvent>("server-status", (e) => {
      const { tag, running, exitCode } = e.payload;
      this.running[tag] = running;
      if (!running && exitCode !== null && exitCode !== undefined) {
        this.pushLog(tag, `[manager] process exited with code ${exitCode}`);
      }
      this.emit();
    });
  }

  pushLog(tag: string, line: string) {
    const arr = this.logs[tag] ?? [];
    arr.push(line);
    // Cap the in-memory buffer.
    this.logs[tag] = arr.length > 2000 ? arr.slice(-1500) : arr;
  }

  /** Notified whenever a server starts/stops. */
  onChange(fn: Listener): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }

  private emit() {
    for (const fn of this.listeners) fn();
  }
}

export const servers = new ServersStore();
