type ToastKind = "info" | "error";

export interface Toast {
  id: number;
  kind: ToastKind;
  text: string;
}

let nextId = 1;

class ToastStore {
  items = $state<Toast[]>([]);

  show(text: string, kind: ToastKind = "info") {
    const id = nextId++;
    this.items.push({ id, kind, text });
    setTimeout(() => this.dismiss(id), 4200);
  }

  error(text: string) {
    this.show(text, "error");
  }

  dismiss(id: number) {
    this.items = this.items.filter((t) => t.id !== id);
  }
}

export const toasts = new ToastStore();
