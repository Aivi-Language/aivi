(function () {
  "use strict";

  function getBoot() {
    var el = document.getElementById("aivi-server-html-boot");
    if (!el) return null;
    try {
      return JSON.parse(el.textContent || "{}");
    } catch (_) {
      return null;
    }
  }

  function resolveWsUrl(wsUrl) {
    if (!wsUrl) return null;
    if (typeof wsUrl !== "string") return null;
    if (wsUrl.indexOf("ws://") === 0 || wsUrl.indexOf("wss://") === 0) return wsUrl;
    if (wsUrl.indexOf("/") === 0) {
      var proto = location.protocol === "https:" ? "wss://" : "ws://";
      return proto + location.host + wsUrl;
    }
    return wsUrl;
  }

  function closestWithAttr(el, attr) {
    while (el && el !== document.body) {
      if (el.getAttribute && el.getAttribute(attr)) return el;
      el = el.parentNode;
    }
    return null;
  }

  var boot = getBoot();
  if (!boot || !boot.viewId) return;
  var viewId = String(boot.viewId);
  var wsUrl = resolveWsUrl(boot.wsUrl || boot.wsPath || "/ws");
  if (!wsUrl) return;

  var socket = new WebSocket(wsUrl);

  function send(obj) {
    try {
      socket.send(JSON.stringify(obj));
    } catch (_) {}
  }

  socket.addEventListener("open", function () {
    send({ t: "hello", viewId: viewId, url: String(location.href), online: !!navigator.onLine });
  });

  var nodeCache = new Map();
  function getNode(nodeId) {
    if (nodeCache.has(nodeId)) return nodeCache.get(nodeId);
    var el = document.querySelector('[data-aivi-node="' + CSS.escape(nodeId) + '"]');
    if (el) nodeCache.set(nodeId, el);
    return el;
  }

  function replaceOuterHtml(nodeId, html) {
    var node = getNode(nodeId);
    if (!node) return;
    var tpl = document.createElement("template");
    tpl.innerHTML = String(html || "");
    var newEl = tpl.content.firstElementChild;
    if (!newEl) return;
    node.replaceWith(newEl);
    nodeCache.clear();
  }

  function applyOp(op) {
    if (!op || typeof op.op !== "string") return;
    if (op.op === "replace") return replaceOuterHtml(op.id, op.html);
    if (op.op === "setText") {
      var n1 = getNode(op.id);
      if (!n1) return;
      n1.textContent = String(op.text || "");
      return;
    }
    if (op.op === "setAttr") {
      var n2 = getNode(op.id);
      if (!n2) return;
      n2.setAttribute(String(op.name || ""), String(op.value || ""));
      return;
    }
    if (op.op === "removeAttr") {
      var n3 = getNode(op.id);
      if (!n3) return;
      n3.removeAttribute(String(op.name || ""));
    }
  }

  var intersection = new Map();
  function intersectionSubscribe(msg) {
    var sid = msg && msg.sid;
    if (typeof sid !== "number") return;
    var opts = (msg && msg.p) || {};
    var targets = (msg && msg.targets) || [];

    var state = intersection.get(sid);
    if (!state) {
      var elementToTid = new WeakMap();
      var tidToNodeId = new Map();
      var pending = [];
      var scheduled = false;

      function flush() {
        scheduled = false;
        if (!pending.length) return;
        var entries = pending;
        pending = [];
        send({ t: "platform", viewId: viewId, kind: "intersection", p: { sid: sid, entries: entries } });
      }

      var observer = new IntersectionObserver(
        function (entries) {
          for (var i = 0; i < entries.length; i++) {
            var e = entries[i];
            var tid = elementToTid.get(e.target);
            if (typeof tid !== "number") continue;
            pending.push({
              tid: tid,
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

      state = { observer: observer, elementToTid: elementToTid, tidToNodeId: tidToNodeId };
      intersection.set(sid, state);
    }

    for (var j = 0; j < targets.length; j++) {
      var t = targets[j];
      if (!t) continue;
      var tid2 = t.tid;
      var nodeId = t.nodeId;
      if (typeof tid2 !== "number" || typeof nodeId !== "string") continue;
      var el = getNode(nodeId);
      state.tidToNodeId.set(tid2, nodeId);
      if (!el) continue;
      state.elementToTid.set(el, tid2);
      state.observer.observe(el);
    }
  }

  function intersectionUnsubscribe(msg) {
    var sid = msg && msg.sid;
    if (typeof sid !== "number") return;
    var state = intersection.get(sid);
    if (!state) return;
    try {
      state.observer.disconnect();
    } catch (_) {}
    intersection.delete(sid);
  }

  function handleEffect(msg) {
    if (!msg || typeof msg.kind !== "string" || typeof msg.rid !== "number") return;
    var rid = msg.rid;
    if (msg.kind === "clipboard.readText") {
      if (!navigator.clipboard || !navigator.clipboard.readText) {
        return send({ t: "effectResult", viewId: viewId, rid: rid, kind: msg.kind, ok: false, error: "Unavailable" });
      }
      navigator.clipboard
        .readText()
        .then(function (text) {
          send({ t: "effectResult", viewId: viewId, rid: rid, kind: msg.kind, ok: true, p: { text: String(text) } });
        })
        .catch(function (err) {
          send({ t: "effectResult", viewId: viewId, rid: rid, kind: msg.kind, ok: false, error: String((err && err.name) || "Error") });
        });
      return;
    }
    if (msg.kind === "clipboard.writeText") {
      if (!navigator.clipboard || !navigator.clipboard.writeText) {
        return send({ t: "effectResult", viewId: viewId, rid: rid, kind: msg.kind, ok: false, error: "Unavailable" });
      }
      var text2 = msg.p && typeof msg.p.text === "string" ? msg.p.text : "";
      navigator.clipboard
        .writeText(text2)
        .then(function () {
          send({ t: "effectResult", viewId: viewId, rid: rid, kind: msg.kind, ok: true, p: {} });
        })
        .catch(function (err2) {
          send({ t: "effectResult", viewId: viewId, rid: rid, kind: msg.kind, ok: false, error: String((err2 && err2.name) || "Error") });
        });
    }
  }

  socket.addEventListener("message", function (ev) {
    var msg = null;
    try {
      msg = JSON.parse(ev.data);
    } catch (_) {
      return;
    }
    if (!msg || msg.viewId !== viewId) return;
    if (msg.t === "patch" && Array.isArray(msg.ops)) {
      for (var i = 0; i < msg.ops.length; i++) applyOp(msg.ops[i]);
      return;
    }
    if (msg.t === "subscribe" && msg.kind === "intersection") return intersectionSubscribe(msg);
    if (msg.t === "unsubscribe" && msg.kind === "intersection") return intersectionUnsubscribe(msg);
    if (msg.t === "effect") return handleEffect(msg);
  });

  function sendDomEvent(kind, ev) {
    var attr = "data-aivi-hid-" + kind;
    var target = ev && ev.target;
    if (target && target.nodeType === 3) target = target.parentNode;
    var el = closestWithAttr(target, attr);
    if (!el) return;
    var hid = parseInt(el.getAttribute(attr), 10);
    if (!isFinite(hid)) return;

    var p = {};
    if (kind === "click") {
      p = { button: ev.button | 0, alt: !!ev.altKey, ctrl: !!ev.ctrlKey, shift: !!ev.shiftKey, meta: !!ev.metaKey };
    } else if (kind === "input") {
      var v = "";
      try {
        if (ev.target && "value" in ev.target) v = String(ev.target.value);
      } catch (_) {}
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
    send({ t: "event", viewId: viewId, hid: hid, kind: kind, p: p });
  }

  document.addEventListener("click", function (ev) { sendDomEvent("click", ev); });
  document.addEventListener("input", function (ev) { sendDomEvent("input", ev); });
  document.addEventListener("keydown", function (ev) { sendDomEvent("keydown", ev); });
  document.addEventListener("keyup", function (ev) { sendDomEvent("keyup", ev); });
  document.addEventListener("pointerdown", function (ev) { sendDomEvent("pointerdown", ev); });
  document.addEventListener("pointerup", function (ev) { sendDomEvent("pointerup", ev); });
  document.addEventListener("pointermove", function (ev) { sendDomEvent("pointermove", ev); });
  document.addEventListener("focus", function (ev) { sendDomEvent("focus", ev); }, true);
  document.addEventListener("blur", function (ev) { sendDomEvent("blur", ev); }, true);

  window.addEventListener("popstate", function () {
    send({ t: "platform", viewId: viewId, kind: "popstate", p: { url: String(location.href), path: String(location.pathname), query: String(location.search), hash: String(location.hash) } });
  });
  window.addEventListener("hashchange", function (ev) {
    send({ t: "platform", viewId: viewId, kind: "hashchange", p: { url: String(location.href), oldURL: String(ev.oldURL || ""), newURL: String(ev.newURL || ""), hash: String(location.hash) } });
  });
  document.addEventListener("visibilitychange", function () {
    send({ t: "platform", viewId: viewId, kind: "visibilitychange", p: { visibilityState: String(document.visibilityState || "") } });
  });
  window.addEventListener("focus", function () {
    send({ t: "platform", viewId: viewId, kind: "focus", p: { focused: true } });
  });
  window.addEventListener("blur", function () {
    send({ t: "platform", viewId: viewId, kind: "blur", p: { focused: false } });
  });
  window.addEventListener("online", function () {
    send({ t: "platform", viewId: viewId, kind: "online", p: { online: true } });
  });
  window.addEventListener("offline", function () {
    send({ t: "platform", viewId: viewId, kind: "online", p: { online: false } });
  });
})();

