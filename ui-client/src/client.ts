/* eslint-disable */

import type { Boot, ClientMsg, ServerMsg } from "./protocol";

function getBoot(): Boot | null {
  const el = document.getElementById("aivi-server-html-boot");
  if (!el) return null;
  try {
    return JSON.parse(el.textContent || "{}") as Boot;
  } catch {
    return null;
  }
}

function resolveWsUrl(wsUrl: unknown): string | null {
  if (typeof wsUrl !== "string" || !wsUrl) return null;
  if (wsUrl.startsWith("ws://") || wsUrl.startsWith("wss://")) return wsUrl;
  if (wsUrl.startsWith("/")) {
    const proto = location.protocol === "https:" ? "wss://" : "ws://";
    return proto + location.host + wsUrl;
  }
  return wsUrl;
}

function closestWithAttr(el: unknown, attr: string): Element | null {
  let cur: any = el;
  while (cur && cur !== document.body) {
    if (cur.getAttribute && cur.getAttribute(attr)) return cur as Element;
    cur = cur.parentNode;
  }
  return null;
}

function start(): void {
  const boot = getBoot();
  if (!boot || !boot.viewId) return;

  const viewId = String(boot.viewId);
  const wsUrl = resolveWsUrl(boot.wsUrl ?? boot.wsPath ?? "/ws");
  if (!wsUrl) return;

  const socket = new WebSocket(wsUrl);

  function send(obj: ClientMsg): void {
    try {
      socket.send(JSON.stringify(obj));
    } catch {
      // ignore
    }
  }

  socket.addEventListener("open", () => {
    send({ t: "hello", viewId, url: String(location.href), online: !!navigator.onLine });
  });

  // Node cache for patch application.
  const nodeCache = new Map<string, Element>(); // nodeId -> Element
  function getNode(nodeId: string): Element | null {
    const cached = nodeCache.get(nodeId);
    if (cached) return cached;
    const el = document.querySelector(`[data-aivi-node="${CSS.escape(nodeId)}"]`);
    if (el) nodeCache.set(nodeId, el);
    return el;
  }

  function replaceOuterHtml(nodeId: string, html: unknown): void {
    const node = getNode(nodeId);
    if (!node) return;
    const tpl = document.createElement("template");
    tpl.innerHTML = String(html || "");
    const newEl = tpl.content.firstElementChild;
    if (!newEl) return;
    node.replaceWith(newEl);
    nodeCache.clear();
  }

  function applyOp(op: any): void {
    if (!op || typeof op.op !== "string") return;
    if (op.op === "replace") return replaceOuterHtml(String(op.id || ""), op.html);
    if (op.op === "setText") {
      const n = getNode(String(op.id || ""));
      if (!n) return;
      n.textContent = String(op.text || "");
      return;
    }
    if (op.op === "setAttr") {
      const n = getNode(String(op.id || ""));
      if (!n) return;
      n.setAttribute(String(op.name || ""), String(op.value || ""));
      return;
    }
    if (op.op === "removeAttr") {
      const n = getNode(String(op.id || ""));
      if (!n) return;
      n.removeAttribute(String(op.name || ""));
    }
  }

  // IntersectionObserver subscriptions keyed by `sid`.
  type IntersectionState = { observer: IntersectionObserver; elementToTid: WeakMap<Element, number>; tidToNodeId: Map<number, string> };
  const intersection = new Map<number, IntersectionState>();

  function intersectionSubscribe(msg: any): void {
    const sid = msg && msg.sid;
    if (typeof sid !== "number") return;
    const opts = (msg && msg.p) || {};
    const targets = (msg && msg.targets) || [];

    let state = intersection.get(sid);
    if (!state) {
      const elementToTid = new WeakMap<Element, number>();
      const tidToNodeId = new Map<number, string>();
      let pending: any[] = [];
      let scheduled = false;

      function flush() {
        scheduled = false;
        if (!pending.length) return;
        const entries = pending;
        pending = [];
        send({ t: "platform", viewId, kind: "intersection", p: { sid, entries } });
      }

      const observer = new IntersectionObserver(
        (entries) => {
          for (const e of entries) {
            const tid = elementToTid.get(e.target);
            if (typeof tid !== "number") continue;
            pending.push({
              tid,
              isIntersecting: !!e.isIntersecting,
              ratio: typeof e.intersectionRatio === "number" ? e.intersectionRatio : 0
            });
          }
          if (!scheduled) {
            scheduled = true;
            Promise.resolve().then(flush);
          }
        },
        {
          root: null,
          rootMargin: typeof opts.rootMargin === "string" ? opts.rootMargin : "0px",
          threshold: Array.isArray(opts.threshold) ? opts.threshold : [0]
        }
      );

      state = { observer, elementToTid, tidToNodeId };
      intersection.set(sid, state);
    }

    for (const t of targets) {
      if (!t) continue;
      const tid = t.tid;
      const nodeId = t.nodeId;
      if (typeof tid !== "number" || typeof nodeId !== "string") continue;
      const el = getNode(nodeId);
      state.tidToNodeId.set(tid, nodeId);
      if (!el) continue;
      state.elementToTid.set(el, tid);
      state.observer.observe(el);
    }
  }

  function intersectionUnsubscribe(msg: any): void {
    const sid = msg && msg.sid;
    if (typeof sid !== "number") return;
    const state = intersection.get(sid);
    if (!state) return;
    try {
      state.observer.disconnect();
    } catch {
      // ignore
    }
    intersection.delete(sid);
  }

  function handleEffect(msg: any): void {
    if (!msg || typeof msg.kind !== "string" || typeof msg.rid !== "number") return;
    const rid = msg.rid;

    if (msg.kind === "clipboard.readText") {
      if (!navigator.clipboard || !navigator.clipboard.readText) {
        return send({ t: "effectResult", viewId, rid, kind: msg.kind, ok: false, error: "Unavailable" });
      }
      navigator.clipboard
        .readText()
        .then((text) => {
          send({ t: "effectResult", viewId, rid, kind: msg.kind, ok: true, p: { text: String(text) } });
        })
        .catch((err) => {
          send({
            t: "effectResult",
            viewId,
            rid,
            kind: msg.kind,
            ok: false,
            error: String((err && (err as any).name) || "Error")
          });
        });
      return;
    }

    if (msg.kind === "clipboard.writeText") {
      if (!navigator.clipboard || !navigator.clipboard.writeText) {
        return send({ t: "effectResult", viewId, rid, kind: msg.kind, ok: false, error: "Unavailable" });
      }
      const text2 = msg.p && typeof msg.p.text === "string" ? msg.p.text : "";
      navigator.clipboard
        .writeText(text2)
        .then(() => {
          send({ t: "effectResult", viewId, rid, kind: msg.kind, ok: true, p: {} });
        })
        .catch((err2) => {
          send({
            t: "effectResult",
            viewId,
            rid,
            kind: msg.kind,
            ok: false,
            error: String((err2 && (err2 as any).name) || "Error")
          });
        });
    }
  }

  socket.addEventListener("message", (ev) => {
    let msg: ServerMsg | null = null;
    try {
      msg = JSON.parse(String((ev as any).data)) as ServerMsg;
    } catch {
      return;
    }
    if (!msg || (msg as any).viewId !== viewId) return;

    if (msg.t === "patch" && Array.isArray((msg as any).ops)) {
      for (const op of (msg as any).ops) applyOp(op);
      return;
    }
    if (msg.t === "subscribe" && (msg as any).kind === "intersection") return intersectionSubscribe(msg);
    if (msg.t === "unsubscribe" && (msg as any).kind === "intersection") return intersectionUnsubscribe(msg);
    if (msg.t === "effect") return handleEffect(msg);
  });

  function sendDomEvent(kind: string, ev: any): void {
    const attr = `data-aivi-hid-${kind}`;
    let target: any = ev && ev.target;
    if (target && target.nodeType === 3) target = target.parentNode;
    const el = closestWithAttr(target, attr);
    if (!el) return;
    const hid = parseInt(String(el.getAttribute(attr)), 10);
    if (!isFinite(hid)) return;

    let p: any = {};
    if (kind === "click") {
      p = { button: ev.button | 0, alt: !!ev.altKey, ctrl: !!ev.ctrlKey, shift: !!ev.shiftKey, meta: !!ev.metaKey };
    } else if (kind === "input") {
      let v = "";
      try {
        if (ev.target && "value" in ev.target) v = String(ev.target.value);
      } catch {
        // ignore
      }
      p = { value: v };
    } else if (kind === "keydown" || kind === "keyup") {
      p = {
        key: String(ev.key || ""),
        code: String(ev.code || ""),
        alt: !!ev.altKey,
        ctrl: !!ev.ctrlKey,
        shift: !!ev.shiftKey,
        meta: !!ev.metaKey,
        repeat: !!ev.repeat,
        isComposing: !!ev.isComposing
      };
    } else if (kind === "pointerdown" || kind === "pointerup" || kind === "pointermove") {
      p = {
        pointerId: ev.pointerId | 0,
        pointerType: String(ev.pointerType || ""),
        button: ev.button | 0,
        buttons: ev.buttons | 0,
        clientX: +ev.clientX || 0,
        clientY: +ev.clientY || 0,
        alt: !!ev.altKey,
        ctrl: !!ev.ctrlKey,
        shift: !!ev.shiftKey,
        meta: !!ev.metaKey
      };
    }
    send({ t: "event", viewId, hid, kind, p });
  }

  // Delegated DOM events. `focus`/`blur` require capture to reach the document listener.
  document.addEventListener("click", (ev) => sendDomEvent("click", ev));
  document.addEventListener("input", (ev) => sendDomEvent("input", ev));
  document.addEventListener("keydown", (ev) => sendDomEvent("keydown", ev));
  document.addEventListener("keyup", (ev) => sendDomEvent("keyup", ev));
  document.addEventListener("pointerdown", (ev) => sendDomEvent("pointerdown", ev));
  document.addEventListener("pointerup", (ev) => sendDomEvent("pointerup", ev));
  document.addEventListener("pointermove", (ev) => sendDomEvent("pointermove", ev));
  document.addEventListener(
    "focus",
    (ev) => {
      sendDomEvent("focus", ev);
    },
    true
  );
  document.addEventListener(
    "blur",
    (ev) => {
      sendDomEvent("blur", ev);
    },
    true
  );

  // Platform signals.
  window.addEventListener("popstate", () => {
    send({
      t: "platform",
      viewId,
      kind: "popstate",
      p: { url: String(location.href), path: String(location.pathname), query: String(location.search), hash: String(location.hash) }
    });
  });
  window.addEventListener("hashchange", (ev: any) => {
    send({
      t: "platform",
      viewId,
      kind: "hashchange",
      p: { url: String(location.href), oldURL: String(ev && ev.oldURL ? ev.oldURL : ""), newURL: String(ev && ev.newURL ? ev.newURL : ""), hash: String(location.hash) }
    });
  });
  document.addEventListener("visibilitychange", () => {
    send({ t: "platform", viewId, kind: "visibilitychange", p: { visibilityState: String((document as any).visibilityState || "") } });
  });
  window.addEventListener("focus", () => {
    send({ t: "platform", viewId, kind: "focus", p: { focused: true } });
  });
  window.addEventListener("blur", () => {
    send({ t: "platform", viewId, kind: "blur", p: { focused: false } });
  });
  window.addEventListener("online", () => {
    send({ t: "platform", viewId, kind: "online", p: { online: true } });
  });
  window.addEventListener("offline", () => {
    send({ t: "platform", viewId, kind: "online", p: { online: false } });
  });
}

start();

