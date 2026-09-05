import {
  createContext,
  useContext,
  useEffect,
  useRef,
  type ComponentPropsWithoutRef,
  type PropsWithChildren,
  type RefObject,
} from "react";

const DialogOpener = createContext<
  RefObject<HTMLElement | undefined> | undefined
>(undefined);

/** WebKit does not focus clicked buttons; remember activation without changing pointer behavior. */
export function DialogHost({ children }: PropsWithChildren) {
  const opener = useRef<HTMLElement | undefined>(undefined);
  useEffect(() => {
    const rememberClick = (event: MouseEvent) => {
      const target =
        event.target instanceof Element
          ? event.target.closest<HTMLElement>(
              "button:not([disabled]), a[href], summary, [role='button']",
            )
          : null;
      if (target) opener.current = target;
    };
    const rememberFocus = (event: FocusEvent) => {
      if (event.target instanceof HTMLElement) opener.current = event.target;
    };
    document.addEventListener("click", rememberClick, true);
    document.addEventListener("focusin", rememberFocus, true);
    return () => {
      document.removeEventListener("click", rememberClick, true);
      document.removeEventListener("focusin", rememberFocus, true);
    };
  }, []);
  return (
    <DialogOpener.Provider value={opener}>{children}</DialogOpener.Provider>
  );
}

interface DialogSurfaceProps extends ComponentPropsWithoutRef<"section"> {
  readonly onDismiss: () => void;
  readonly dismissible?: boolean;
  readonly modal?: boolean;
}

const layers: HTMLElement[] = [];
const focusableSelector = [
  "button:not([disabled])",
  "a[href]",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "summary",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

/** Closing a surface only closes UI; operation cancellation is always an explicit command. */
export function DialogSurface({
  children,
  onDismiss,
  dismissible = true,
  modal = true,
  ...props
}: DialogSurfaceProps) {
  const element = useRef<HTMLElement>(null);
  const opener = useContext(DialogOpener);
  const behavior = useRef({ onDismiss, dismissible, modal });
  behavior.current = { onDismiss, dismissible, modal };

  useEffect(() => {
    const surface = element.current;
    if (!surface) return;
    const previous =
      opener?.current?.isConnected && !surface.contains(opener.current)
        ? opener.current
        : document.activeElement instanceof HTMLElement
          ? document.activeElement
          : undefined;
    layers.push(surface);
    const focusables = () =>
      [...surface.querySelectorAll<HTMLElement>(focusableSelector)].filter(
        (node) =>
          node.tabIndex >= 0 &&
          node.getClientRects().length > 0 &&
          !node.matches(":disabled") &&
          !node.closest("[hidden], [inert]") &&
          !["hidden", "collapse"].includes(getComputedStyle(node).visibility),
      );
    // Focus the surface, never a motion/start button. Enter must not start work on open.
    surface.focus({ preventScroll: true });
    const keydown = (event: KeyboardEvent) => {
      if (layers.at(-1) !== surface || event.isComposing) return;
      if (event.key === "Escape") {
        event.stopPropagation();
        event.preventDefault();
        if (behavior.current.dismissible) behavior.current.onDismiss();
      } else if (event.key === "Tab" && behavior.current.modal) {
        const items = focusables();
        const first = items[0];
        const last = items.at(-1);
        const active = document.activeElement;
        if (!first) {
          event.preventDefault();
          surface.focus({ preventScroll: true });
        } else if (
          event.shiftKey &&
          (active === first || !items.includes(active as HTMLElement))
        ) {
          event.preventDefault();
          last?.focus();
        } else if (
          !event.shiftKey &&
          (active === last || !items.includes(active as HTMLElement))
        ) {
          event.preventDefault();
          first.focus();
        }
      }
    };
    document.addEventListener("keydown", keydown, true);
    return () => {
      document.removeEventListener("keydown", keydown, true);
      const wasTop = layers.at(-1) === surface;
      const index = layers.indexOf(surface);
      if (index >= 0) layers.splice(index, 1);
      const active = document.activeElement;
      if (
        wasTop &&
        previous?.isConnected &&
        (active === document.body || surface.contains(active))
      )
        previous.focus({ preventScroll: true });
    };
  }, []);

  return (
    <section
      {...props}
      ref={element}
      role="dialog"
      aria-modal={modal}
      tabIndex={-1}
    >
      {children}
    </section>
  );
}
