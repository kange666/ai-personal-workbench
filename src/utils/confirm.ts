export type ConfirmTone = "primary" | "warning" | "danger";

export interface ConfirmOptions {
  title?: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  tone?: ConfirmTone;
}

export interface ConfirmRequest extends Required<ConfirmOptions> {
  id: number;
  respond: (accepted: boolean) => void;
}

type ConfirmListener = (request: ConfirmRequest) => void;

interface QueuedConfirm {
  id: number;
  options: Required<ConfirmOptions>;
  resolve: (accepted: boolean) => void;
}

let nextId = 1;
let listener: ConfirmListener | null = null;
let active = false;
const queue: QueuedConfirm[] = [];

function normalize(options: ConfirmOptions | string): Required<ConfirmOptions> {
  const value = typeof options === "string" ? { message: options } : options;
  return {
    title: value.title || "请确认操作",
    message: value.message,
    confirmText: value.confirmText || "确认",
    cancelText: value.cancelText || "取消",
    tone: value.tone || "primary",
  };
}

function dispatchNext() {
  if (!listener || active || !queue.length) return;
  active = true;
  const current = queue[0];
  let answered = false;
  listener({
    id: current.id,
    ...current.options,
    respond(accepted) {
      if (answered) return;
      answered = true;
      current.resolve(accepted);
      queue.shift();
      active = false;
      dispatchNext();
    },
  });
}

export function confirmAction(options: ConfirmOptions | string): Promise<boolean> {
  return new Promise((resolve) => {
    queue.push({ id: nextId++, options: normalize(options), resolve });
    dispatchNext();
  });
}

export function subscribeConfirm(listenerValue: ConfirmListener): () => void {
  listener = listenerValue;
  dispatchNext();
  return () => {
    if (listener === listenerValue) listener = null;
  };
}

export function resetConfirmQueueForTests() {
  while (queue.length) queue.shift()?.resolve(false);
  active = false;
  listener = null;
  nextId = 1;
}
