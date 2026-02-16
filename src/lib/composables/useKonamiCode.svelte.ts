/**
 * Konami Code detector composable
 * Detects the famous cheat code: ↑↑↓↓←→←→BA
 */

const KONAMI_CODE = [
  'ArrowUp', 'ArrowUp', 'ArrowDown', 'ArrowDown',
  'ArrowLeft', 'ArrowRight', 'ArrowLeft', 'ArrowRight',
  'KeyB', 'KeyA'
];

/**
 * Creates a Konami Code detector that calls the provided callback when activated
 *
 * Usage:
 * ```
 * let showSecret = $state(false);
 * useKonamiCode(() => { showSecret = true; });
 * ```
 */
export function useKonamiCode(onActivate: () => void) {
  let sequence: string[] = [];
  let timeoutId: ReturnType<typeof setTimeout> | null = null;

  function handleKeydown(e: KeyboardEvent) {
    // Don't track if user is typing in an input
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
      return;
    }

    // Clear sequence after 2 seconds of inactivity
    if (timeoutId) {
      clearTimeout(timeoutId);
    }
    timeoutId = setTimeout(() => {
      sequence = [];
    }, 2000);

    // Add key to sequence, keep only last 10 keys
    sequence = [...sequence, e.code].slice(-KONAMI_CODE.length);

    // Check if sequence matches
    if (sequence.length === KONAMI_CODE.length &&
        sequence.every((key, i) => key === KONAMI_CODE[i])) {
      onActivate();
      sequence = [];
      if (timeoutId) {
        clearTimeout(timeoutId);
        timeoutId = null;
      }
    }
  }

  // Set up listener on mount, clean up on unmount
  if (typeof window !== 'undefined') {
    window.addEventListener('keydown', handleKeydown);
  }

  return {
    destroy() {
      if (typeof window !== 'undefined') {
        window.removeEventListener('keydown', handleKeydown);
      }
      if (timeoutId) {
        clearTimeout(timeoutId);
      }
    }
  };
}
