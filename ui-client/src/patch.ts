// ---------------------------------------------------------------------------
// patch.ts — DOM patch application
// ---------------------------------------------------------------------------

/** Cache of nodeId → Element for fast lookups. */
const nodeCache = new Map<string, Element>();

/** Look up an element by its `data-aivi-node` attribute. */
function getNode(nodeId: string): Element | null {
  const cached = nodeCache.get(nodeId);
  if (cached && cached.isConnected) return cached;
  const el = document.querySelector(`[data-aivi-node="${CSS.escape(nodeId)}"]`);
  if (el) nodeCache.set(nodeId, el);
  return el;
}

/** Replace an element's entire outer HTML. Clears the node cache. */
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

/** Apply a single patch operation. */
function applyOp(op: any): void {
  if (!op || typeof op.op !== "string") return;
  switch (op.op) {
    case "replace":
      replaceOuterHtml(String(op.id || ""), op.html);
      break;
    case "setText": {
      const n = getNode(String(op.id || ""));
      if (n) n.textContent = String(op.text || "");
      break;
    }
    case "setAttr": {
      const n = getNode(String(op.id || ""));
      if (n) n.setAttribute(String(op.name || ""), String(op.value || ""));
      break;
    }
    case "removeAttr": {
      const n = getNode(String(op.id || ""));
      if (n) n.removeAttribute(String(op.name || ""));
      break;
    }
  }
}

/**
 * Apply a batch of patch operations received as a JSON-encoded string.
 * The string may be a JSON array OR a pre-parsed array (if the server
 * wraps ops in the `ops` field of the patch message).
 */
export function applyPatches(opsJson: string): void {
  let ops: unknown[];
  try {
    ops = JSON.parse(opsJson);
  } catch {
    return;
  }
  if (!Array.isArray(ops)) return;
  for (const op of ops) applyOp(op);
}

export { getNode };
