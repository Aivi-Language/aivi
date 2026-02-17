export type Boot = {
  viewId: string;
  wsUrl?: string;
  wsPath?: string;
};

export type PatchOp =
  | { op: "replace"; id: string; html: string }
  | { op: "setText"; id: string; text: string }
  | { op: "setAttr"; id: string; name: string; value: string }
  | { op: "removeAttr"; id: string; name: string };

export type ServerMsg =
  | { t: "patch"; viewId: string; ops: PatchOp[] }
  | { t: "subscribe"; viewId: string; kind: "intersection"; sid: number; p: unknown; targets: unknown[] }
  | { t: "unsubscribe"; viewId: string; kind: "intersection"; sid: number }
  | { t: "effect"; viewId: string; rid: number; kind: string; p?: any }
  | { t: "error"; viewId: string; message: string; code: string };

export type ClientMsg =
  | { t: "hello"; viewId: string; url: string; online: boolean }
  | { t: "event"; viewId: string; hid: number; kind: string; p: unknown }
  | { t: "platform"; viewId: string; kind: string; p: unknown }
  | { t: "effectResult"; viewId: string; rid: number; kind: string; ok: boolean; p?: any; error?: string };

