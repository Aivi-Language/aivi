// ---------------------------------------------------------------------------
// main.ts — Entry point (IIFE side-effect)
// ---------------------------------------------------------------------------

import { initWs, send } from "./ws";
import { installEventListeners } from "./events";

// -- Boot -------------------------------------------------------------------

/** Build a UrlInfo payload from the current window.location. */
function buildUrlInfo(): { href: string; path: string; query: string; hash: string } {
  return {
    href: String(location.href),
    path: String(location.pathname),
    query: String(location.search),
    hash: String(location.hash),
  };
}

/** Read the viewId from the boot blob (duplicated from ws.ts for platform listeners). */
function getViewId(): string {
  const el = document.getElementById("aivi-server-html-boot");
  if (!el) return "";
  try {
    const boot = JSON.parse(el.textContent || "{}");
    return String(boot.viewId || "");
  } catch {
    return "";
  }
}

function start(): void {
  // 1. Initialize WebSocket
  initWs();

  // 2. Install delegated DOM event listeners
  const viewId = getViewId();
  installEventListeners((hid, kind, payload) => {
    send({ t: "event", viewId, hid, kind, p: payload });
  });

  // 3. Install platform listeners

  window.addEventListener("popstate", () => {
    send({
      t: "platform",
      viewId,
      kind: "popstate",
      p: buildUrlInfo(),
    });
  });

  window.addEventListener("hashchange", (ev: HashChangeEvent) => {
    send({
      t: "platform",
      viewId,
      kind: "hashchange",
      p: {
        url: buildUrlInfo(),
        oldURL: String(ev.oldURL || ""),
        newURL: String(ev.newURL || ""),
        hash: String(location.hash),
      },
    });
  });

  document.addEventListener("visibilitychange", () => {
    send({
      t: "platform",
      viewId,
      kind: "visibility",
      p: { state: String(document.visibilityState || "") },
    });
  });

  window.addEventListener("focus", () => {
    send({
      t: "platform",
      viewId,
      kind: "focus",
      p: { focused: true },
    });
  });

  window.addEventListener("blur", () => {
    send({
      t: "platform",
      viewId,
      kind: "focus",
      p: { focused: false },
    });
  });

  window.addEventListener("online", () => {
    send({
      t: "platform",
      viewId,
      kind: "online",
      p: { online: true },
    });
  });

  window.addEventListener("offline", () => {
    send({
      t: "platform",
      viewId,
      kind: "online",
      p: { online: false },
    });
  });
}

start();
