import type { ControllerSnapshot } from "./machine";

export const isControllerConnected = (snapshot: ControllerSnapshot): boolean =>
  snapshot.connection === "connected";

export const hasControllerSession = (snapshot: ControllerSnapshot): boolean =>
  snapshot.connection === "connected" || snapshot.connection === "recovering";

export const isControllerStableIdle = (snapshot: ControllerSnapshot): boolean =>
  isControllerConnected(snapshot) &&
  snapshot.machine.mode === "idle" &&
  snapshot.alarm === undefined &&
  snapshot.resetNotice === undefined;
