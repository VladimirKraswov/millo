/** Global motion shortcuts must yield to focused UI and already-handled keyboard events. */
export function acceptsKeyboardJogEvent(event: KeyboardEvent): boolean {
  if (event.defaultPrevented || event.repeat || event.isComposing || event.altKey || event.ctrlKey || event.metaKey) return false;
  const target = event.target instanceof Element ? event.target : undefined;
  return !target?.closest(
    "input, textarea, select, [contenteditable]:not([contenteditable='false']), [role='textbox'], [role='dialog'], [role='menu'], [role='listbox']",
  );
}
