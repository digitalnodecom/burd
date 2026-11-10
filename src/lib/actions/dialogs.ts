/** Apply consistent keyboard and focus behavior to dialogs, including nested dialogs. */
export function dialogs(root: HTMLElement) {
  const previous = new Map<HTMLElement, HTMLElement | null>();
  const focusable = (dialog: HTMLElement) => Array.from(dialog.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), a[href], [tabindex="0"]')).filter(el => el.getClientRects().length > 0);
  const sync = () => {
    for (const [dialog, trigger] of previous) {
      if (!dialog.isConnected) { previous.delete(dialog); trigger?.focus(); }
    }
    root.querySelectorAll<HTMLElement>('[role="dialog"]').forEach(dialog => {
      if (previous.has(dialog)) return;
      previous.set(dialog, document.activeElement as HTMLElement | null);
      const heading = dialog.querySelector('h2, h3');
      if (heading && !dialog.hasAttribute('aria-label')) dialog.setAttribute('aria-label', heading.textContent ?? 'Dialog');
      queueMicrotask(() => (focusable(dialog)[0] ?? dialog).focus());
    });
  };
  const keydown = (event: KeyboardEvent) => {
    const dialog = Array.from(previous.keys()).at(-1);
    if (!dialog) return;
    if (event.key === 'Escape' && !dialog.contains(event.target as Node)) {
      dialog.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
      event.preventDefault();
    }
    if (event.key === 'Tab') {
      const items = focusable(dialog);
      const index = items.indexOf(document.activeElement as HTMLElement);
      if (!items.length) { event.preventDefault(); dialog.focus(); }
      else if (event.shiftKey && index <= 0) { event.preventDefault(); items.at(-1)?.focus(); }
      else if (!event.shiftKey && (index < 0 || index === items.length - 1)) { event.preventDefault(); items[0].focus(); }
    }
  };
  const observer = new MutationObserver(sync);
  observer.observe(root, { childList: true, subtree: true });
  document.addEventListener('keydown', keydown, true);
  sync();
  return { destroy() { observer.disconnect(); document.removeEventListener('keydown', keydown, true); } };
}
