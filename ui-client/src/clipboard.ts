// ---------------------------------------------------------------------------
// clipboard.ts — Clipboard effect executor
// ---------------------------------------------------------------------------

import type { ClientMsg, ClipboardOpWire } from "./types";

function errorName(e: unknown): string {
  if (e && typeof e === "object" && "name" in e) return String((e as any).name);
  return "Error";
}

/**
 * Execute a clipboard effect request and send the result back to the server.
 *
 * @param rid   Request id from the server
 * @param op    The clipboard operation descriptor
 * @param send  Callback to send the EffectResult message
 * @param viewId  Current view id
 */
export function handleClipboardEffect(
  rid: number,
  op: ClipboardOpWire,
  send: (msg: ClientMsg) => void,
  viewId: string,
): void {
  const kind = op.kind;

  if (kind === "clipboard.readText") {
    if (!navigator.clipboard || !navigator.clipboard.readText) {
      send({
        t: "effectResult",
        viewId,
        rid,
        kind,
        ok: false,
        error: "Unavailable",
      });
      return;
    }
    navigator.clipboard
      .readText()
      .then((text) => {
        send({ t: "effectResult", viewId, rid, kind, ok: true, p: { text } });
      })
      .catch((e) => {
        send({
          t: "effectResult",
          viewId,
          rid,
          kind,
          ok: false,
          error: errorName(e),
        });
      });
    return;
  }

  if (kind === "clipboard.writeText") {
    if (!navigator.clipboard || !navigator.clipboard.writeText) {
      send({
        t: "effectResult",
        viewId,
        rid,
        kind,
        ok: false,
        error: "Unavailable",
      });
      return;
    }
    const text = op.text ?? "";
    navigator.clipboard
      .writeText(text)
      .then(() => {
        send({ t: "effectResult", viewId, rid, kind, ok: true, p: {} });
      })
      .catch((e) => {
        send({
          t: "effectResult",
          viewId,
          rid,
          kind,
          ok: false,
          error: errorName(e),
        });
      });
  }
}
