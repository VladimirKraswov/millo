import { expect, it, vi } from "vitest";

import { notifyListeners } from "./notifyListeners";

it("takes a publication snapshot but respects unsubscription during delivery", () => {
  const late = vi.fn();
  const removed = vi.fn();
  const listeners = new Set<(value: number) => void>();
  listeners.add(() => {
    listeners.add(late);
    listeners.delete(removed);
  });
  listeners.add(removed);
  notifyListeners(listeners, 1);
  expect(late).not.toHaveBeenCalled();
  expect(removed).not.toHaveBeenCalled();
  notifyListeners(listeners, 2);
  expect(late).toHaveBeenCalledExactlyOnceWith(2);
});
