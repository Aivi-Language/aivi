// ---------------------------------------------------------------------------
// events.ts — Delegated DOM listeners + payload extractors
// ---------------------------------------------------------------------------

import type {
  AnimationPayload,
  ClickPayload,
  InputPayload,
  KeyPayload,
  PointerPayload,
  TransitionPayload,
} from "./types";

/** Walk up from target to find nearest element with the given attribute. */
function closestWithAttr(el: unknown, attr: string): Element | null {
  let cur: any = el;
  while (cur && cur !== document.body && cur !== document) {
    if (cur.getAttribute && cur.getAttribute(attr)) return cur as Element;
    cur = cur.parentNode;
  }
  return null;
}

// -- Payload extractors (field names match AIVI payload records exactly) ----

export function extractClick(e: MouseEvent): ClickPayload {
  return {
    button: e.button | 0,
    alt: !!e.altKey,
    ctrl: !!e.ctrlKey,
    shift: !!e.shiftKey,
    meta: !!e.metaKey,
  };
}

export function extractInput(e: Event): InputPayload {
  let value = "";
  try {
    const t = e.target as any;
    if (t && "value" in t) value = String(t.value);
  } catch {
    // ignore
  }
  return { value };
}

export function extractKey(e: KeyboardEvent): KeyPayload {
  return {
    key: String(e.key || ""),
    code: String(e.code || ""),
    alt: !!e.altKey,
    ctrl: !!e.ctrlKey,
    shift: !!e.shiftKey,
    meta: !!e.metaKey,
    repeat: !!e.repeat,
    isComposing: !!e.isComposing,
  };
}

export function extractPointer(e: PointerEvent): PointerPayload {
  return {
    pointerId: e.pointerId | 0,
    pointerType: String(e.pointerType || ""),
    button: e.button | 0,
    buttons: e.buttons | 0,
    clientX: +e.clientX || 0,
    clientY: +e.clientY || 0,
    alt: !!e.altKey,
    ctrl: !!e.ctrlKey,
    shift: !!e.shiftKey,
    meta: !!e.metaKey,
  };
}

export function extractTransition(e: TransitionEvent): TransitionPayload {
  return {
    propertyName: String(e.propertyName || ""),
    elapsedTime: Number(e.elapsedTime || 0),
    pseudoElement: String(e.pseudoElement || ""),
  };
}

export function extractAnimation(e: AnimationEvent): AnimationPayload {
  return {
    animationName: String(e.animationName || ""),
    elapsedTime: Number(e.elapsedTime || 0),
    pseudoElement: String(e.pseudoElement || ""),
  };
}

type ExtractorMap = Record<string, (e: any) => unknown>;

const extractors: ExtractorMap = {
  click: extractClick,
  input: extractInput,
  keydown: extractKey,
  keyup: extractKey,
  pointerdown: extractPointer,
  pointerup: extractPointer,
  pointermove: extractPointer,
  transitionend: extractTransition,
  animationend: extractAnimation,
};

/** Supported event kinds. */
export const EVENT_KINDS = [
  "click",
  "input",
  "keydown",
  "keyup",
  "pointerdown",
  "pointerup",
  "pointermove",
  "transitionend",
  "animationend",
] as const;

/**
 * Install ONE delegated listener per event kind on `document`.
 * `sendEvent` is called with `(hid, kind, payload)` when a matching
 * handler id is found.
 */
export function installEventListeners(
  sendEvent: (hid: number, kind: string, payload: unknown) => void,
): void {
  function handleDomEvent(kind: string, ev: Event): void {
    const attr = `data-aivi-hid-${kind}`;
    let target: any = ev.target;
    if (target && target.nodeType === 3) target = target.parentNode;
    const el = closestWithAttr(target, attr);
    if (!el) return;
    const hid = parseInt(String(el.getAttribute(attr)), 10);
    if (!isFinite(hid)) return;
    const extractor = extractors[kind];
    const payload = extractor ? extractor(ev) : {};
    sendEvent(hid, kind, payload);
  }

  for (const kind of EVENT_KINDS) {
    // focus/blur need capturing to reach the document listener.
    document.addEventListener(kind, (ev) => handleDomEvent(kind, ev));
  }
  document.addEventListener("focus", (ev) => handleDomEvent("focus", ev), true);
  document.addEventListener("blur", (ev) => handleDomEvent("blur", ev), true);
}
