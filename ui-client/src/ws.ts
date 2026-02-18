// ---------------------------------------------------------------------------
// ws.ts — WebSocket lifecycle + reconnect
// ---------------------------------------------------------------------------

import type { Boot, ClientMsg, ServerMsg } from "./types";
import { applyPatches } from "./patch";
import { subscribeIntersect, unsubscribeIntersect } from "./intersection";
import { handleClipboardEffect } from "./clipboard";

/** Read the boot blob from the DOM. */
function getBoot(): Boot | null {
  const el = document.getElementById("aivi-server-html-boot");
  if (!el) return null;
  try {
    return JSON.parse(el.textContent || "{}") as Boot;
  } catch {
    return null;
  }
}

/** Resolve a wsUrl (relative or absolute) to a full WebSocket URL. */
function resolveWsUrl(wsUrl: unknown): string | null {
  if (typeof wsUrl !== "string" || !wsUrl) return null;
  if (wsUrl.startsWith("ws://") || wsUrl.startsWith("wss://")) return wsUrl;
  if (wsUrl.startsWith("/")) {
    const proto = location.protocol === "https:" ? "wss://" : "ws://";
    return proto + location.host + wsUrl;
  }
  return wsUrl;
}

// -- State ------------------------------------------------------------------

let viewId = "";
let wsUrl: string | null = null;
let socket: WebSocket | null = null;
let reconnectDelay = 1000;
const MAX_RECONNECT_DELAY = 30000;
const queue: ClientMsg[] = [];

// -- Public API -------------------------------------------------------------

/** Send a ClientMsg to the server. Queues if not yet connected. */
export function send(msg: ClientMsg): void {
  if (socket && socket.readyState === WebSocket.OPEN) {
    try {
      socket.send(JSON.stringify(msg));
    } catch {
      // ignore
    }
  } else {
    queue.push(msg);
  }
}

/** Flush any queued messages. */
function flushQueue(): void {
  while (queue.length > 0) {
    const msg = queue.shift()!;
    send(msg);
  }
}

// -- Message handler --------------------------------------------------------

function handleServerMsg(msg: ServerMsg): void {
  switch (msg.t) {
    case "patch":
      applyPatches(msg.ops);
      break;

    case "error":
      console.warn("[aivi] server error:", msg.code, msg.detail);
      break;

    case "subscribeIntersect":
      subscribeIntersect(msg.sid, msg.options, msg.targets, send, viewId);
      break;

    case "unsubscribeIntersect":
      unsubscribeIntersect(msg.sid);
      break;

    case "effectReq":
      handleClipboardEffect(msg.rid, msg.op, send, viewId);
      break;

    default: {
      // Exhaustiveness guard — should never reach here if ServerMsg is correct.
      const _never: never = msg;
      console.warn("[aivi] unknown server msg type:", (_never as any).t);
    }
  }
}

// -- Connection lifecycle ---------------------------------------------------

function connect(): void {
  if (!wsUrl) return;
  socket = new WebSocket(wsUrl);

  socket.addEventListener("open", () => {
    reconnectDelay = 1000;
    send({ t: "hello", viewId, url: String(location.href), online: !!navigator.onLine });
    flushQueue();
  });

  socket.addEventListener("message", (ev) => {
    let msg: ServerMsg | null = null;
    try {
      msg = JSON.parse(String(ev.data)) as ServerMsg;
    } catch {
      return;
    }
    if (!msg) return;
    handleServerMsg(msg);
  });

  socket.addEventListener("close", () => {
    socket = null;
    scheduleReconnect();
  });

  socket.addEventListener("error", () => {
    // close will fire after error; reconnect happens there.
  });
}

function scheduleReconnect(): void {
  const delay = reconnectDelay;
  reconnectDelay = Math.min(reconnectDelay * 2, MAX_RECONNECT_DELAY);
  setTimeout(connect, delay);
}

// -- Init -------------------------------------------------------------------

/** Initialize the WebSocket connection from the boot blob. */
export function initWs(): void {
  const boot = getBoot();
  if (!boot || !boot.viewId) return;
  viewId = String(boot.viewId);
  wsUrl = resolveWsUrl(boot.wsUrl ?? boot.wsPath ?? "/aivi/live");
  if (!wsUrl) return;
  connect();
}
