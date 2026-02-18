// ---------------------------------------------------------------------------
// intersection.ts — IntersectionObserver manager
// ---------------------------------------------------------------------------

import type { IntersectionOptionsWire, IntersectionTargetWire, ClientMsg } from "./types";
import { getNode } from "./patch";

interface IntersectionState {
  observer: IntersectionObserver;
  elementToTid: WeakMap<Element, number>;
  tidToNodeId: Map<number, string>;
}

/** sid → observer state */
const obsMap = new Map<number, IntersectionState>();

/**
 * Subscribe to intersection observations for a set of targets.
 *
 * @param sid         Subscription id
 * @param options     Observer options (rootMargin, threshold)
 * @param targets     List of { tid, nodeId } to observe
 * @param sendPlatform  Callback to send a platform message to the server
 */
export function subscribeIntersect(
  sid: number,
  options: IntersectionOptionsWire,
  targets: IntersectionTargetWire[],
  sendPlatform: (msg: ClientMsg) => void,
  viewId: string,
): void {
  let state = obsMap.get(sid);

  if (!state) {
    const elementToTid = new WeakMap<Element, number>();
    const tidToNodeId = new Map<number, string>();
    let pending: Array<{ tid: number; isIntersecting: boolean; ratio: number }> = [];
    let scheduled = false;

    function flush(): void {
      scheduled = false;
      if (!pending.length) return;
      const entries = pending;
      pending = [];
      sendPlatform({
        t: "platform",
        viewId,
        kind: "intersection",
        p: { sid, entries },
      });
    }

    const observer = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          const tid = elementToTid.get(e.target);
          if (typeof tid !== "number") continue;
          pending.push({
            tid,
            isIntersecting: !!e.isIntersecting,
            ratio: typeof e.intersectionRatio === "number" ? e.intersectionRatio : 0,
          });
        }
        if (!scheduled) {
          scheduled = true;
          requestAnimationFrame(flush);
        }
      },
      {
        root: null,
        rootMargin: typeof options.rootMargin === "string" ? options.rootMargin : "0px",
        threshold: Array.isArray(options.threshold) ? options.threshold : [0],
      },
    );

    state = { observer, elementToTid, tidToNodeId };
    obsMap.set(sid, state);
  }

  for (const t of targets) {
    if (!t || typeof t.tid !== "number" || typeof t.nodeId !== "string") continue;
    state.tidToNodeId.set(t.tid, t.nodeId);
    const el = getNode(t.nodeId);
    if (!el) continue;
    state.elementToTid.set(el, t.tid);
    state.observer.observe(el);
  }
}

/**
 * Unsubscribe from intersection observations.
 */
export function unsubscribeIntersect(sid: number): void {
  const state = obsMap.get(sid);
  if (!state) return;
  try {
    state.observer.disconnect();
  } catch {
    // ignore
  }
  obsMap.delete(sid);
}
