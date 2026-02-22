// ---------------------------------------------------------------------------
// types.ts — Mirror of Protocol.aivi wire types
// ---------------------------------------------------------------------------

// Discriminator field is always "t"

// -- Client → Server --------------------------------------------------------

export interface HelloMsg {
  t: "hello";
  viewId: string;
  url: string;
  online: boolean;
}

export interface EventMsg {
  t: "event";
  viewId: string;
  hid: number;
  kind: string;
  p: unknown;
}

export interface PlatformMsg {
  t: "platform";
  viewId: string;
  kind: string;
  p: unknown;
}

export interface EffectResultMsg {
  t: "effectResult";
  viewId: string;
  rid: number;
  kind: string;
  ok: boolean;
  p?: unknown;
  error?: string;
}

export type ClientMsg = HelloMsg | EventMsg | PlatformMsg | EffectResultMsg;

// -- Server → Client --------------------------------------------------------

export interface PatchMsg {
  t: "patch";
  ops: string;
}

export interface ErrorMsg {
  t: "error";
  code: string;
  detail: string;
}

export interface SubscribeIntersectMsg {
  t: "subscribeIntersect";
  sid: number;
  options: IntersectionOptionsWire;
  targets: IntersectionTargetWire[];
}

export interface UnsubscribeIntersectMsg {
  t: "unsubscribeIntersect";
  sid: number;
}

export interface EffectReqMsg {
  t: "effectReq";
  rid: number;
  op: ClipboardOpWire;
}

export type ServerMsg =
  | PatchMsg
  | ErrorMsg
  | SubscribeIntersectMsg
  | UnsubscribeIntersectMsg
  | EffectReqMsg;

// -- Shared wire records ----------------------------------------------------

export interface IntersectionOptionsWire {
  rootMargin: string;
  threshold: number[];
}

export interface IntersectionTargetWire {
  tid: number;
  nodeId: string;
}

export interface ClipboardOpWire {
  kind: string;
  text?: string;
}

// -- Event payload records (must match AIVI field names exactly) -------------

export interface ClickPayload {
  button: number;
  alt: boolean;
  ctrl: boolean;
  shift: boolean;
  meta: boolean;
}

export interface InputPayload {
  value: string;
}

export interface KeyPayload {
  key: string;
  code: string;
  alt: boolean;
  ctrl: boolean;
  shift: boolean;
  meta: boolean;
  repeat: boolean;
  isComposing: boolean;
}

export interface PointerPayload {
  pointerId: number;
  pointerType: string;
  button: number;
  buttons: number;
  clientX: number;
  clientY: number;
  alt: boolean;
  ctrl: boolean;
  shift: boolean;
  meta: boolean;
}

export interface TransitionPayload {
  propertyName: string;
  elapsedTime: number;
  pseudoElement: string;
}

export interface AnimationPayload {
  animationName: string;
  elapsedTime: number;
  pseudoElement: string;
}

// -- Boot blob --------------------------------------------------------------

export interface Boot {
  viewId: string;
  wsUrl?: string;
  wsPath?: string;
}
