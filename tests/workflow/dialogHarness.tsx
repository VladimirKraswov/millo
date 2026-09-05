import { StrictMode, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { DialogHost, DialogSurface } from "../../src/components/DialogSurface";
import { acceptsKeyboardJogEvent } from "../../src/features/jog-pad/keyboardJogModel";

export function mount() {
  const host = document.createElement("div");
  document.body.replaceChildren(host);
  createRoot(host).render(
    <StrictMode>
      <DialogHost>
        <Harness />
      </DialogHost>
    </StrictMode>,
  );
}

function Harness() {
  const [open, setOpen] = useState(false);
  const [nested, setNested] = useState(false);
  const [locked, setLocked] = useState(false);
  const [modal, setModal] = useState(true);
  const [jogRequests, setJogRequests] = useState(0);
  useEffect(() => {
    const keydown = (event: KeyboardEvent) => {
      if (event.code === "ArrowUp" && acceptsKeyboardJogEvent(event)) setJogRequests(value => value + 1);
    };
    window.addEventListener("keydown", keydown);
    return () => window.removeEventListener("keydown", keydown);
  }, []);
  return (
    <>
      <button onClick={() => setOpen(true)}>Open surface</button>
      <button onClick={() => { setModal(false); setOpen(true); }}>Open panel</button>
      <button>Outside</button>
      <button onKeyDown={event => event.preventDefault()}>Widget</button>
      <output aria-label="Keyboard jog requests">{jogRequests}</output>
      {open && (
        <DialogSurface
          aria-label="Parent"
          onDismiss={() => setOpen(false)}
          dismissible={!locked}
          modal={modal}
        >
          <input aria-label="First input" />
          <button disabled>Disabled</button>
          <button hidden>Hidden</button>
          <button onClick={() => setNested(true)}>Nested</button>
          <label>
            <input
              type="checkbox"
              checked={locked}
              onChange={(e) => setLocked(e.target.checked)}
            />
            Lock dismissal
          </label>
          <button onClick={() => setOpen(false)}>Close parent</button>
          <fieldset disabled><input aria-label="Disabled fieldset input" /></fieldset>
          <button style={{ visibility: "hidden" }}>Invisible control</button>
          {nested && (
            <DialogSurface
              aria-label="Child"
              onDismiss={() => setNested(false)}
            >
              <button onClick={() => setNested(false)}>Close child</button>
            </DialogSurface>
          )}
        </DialogSurface>
      )}
    </>
  );
}
