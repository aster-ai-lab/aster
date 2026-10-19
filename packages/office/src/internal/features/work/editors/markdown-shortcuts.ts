import type { MarkdownSourceCommand } from './markdown-source-commands';

/** Keyboard event fields shared by DOM and React events. */
export interface MarkdownShortcutEvent {
  altKey: boolean;
  code: string;
  ctrlKey: boolean;
  key: string;
  metaKey: boolean;
  shiftKey: boolean;
}

/**
 * Formatting chords that match the visual editor (TipTap defaults, Notion /
 * Google Docs). Cmd/Ctrl+S is save and is handled separately.
 * Bold, italic, undo, and redo stay on their existing handlers.
 */
export function markdownFormattingShortcut(
  event: MarkdownShortcutEvent,
): MarkdownSourceCommand | null {
  if (!(event.metaKey || event.ctrlKey) || (event.altKey && event.shiftKey)) {
    return null;
  }
  const key = event.key.toLowerCase();
  if (event.altKey) {
    if (event.code === 'Digit0' || event.code === 'Numpad0' || key === '0') {
      return 'paragraph';
    }
    if (event.code === 'Digit1' || event.code === 'Numpad1' || key === '1') {
      return 'heading-1';
    }
    if (event.code === 'Digit2' || event.code === 'Numpad2' || key === '2') {
      return 'heading-2';
    }
    if (event.code === 'Digit3' || event.code === 'Numpad3' || key === '3') {
      return 'heading-3';
    }
    if (event.code === 'Digit4' || event.code === 'Numpad4' || key === '4') {
      return 'heading-4';
    }
    if (event.code === 'Digit5' || event.code === 'Numpad5' || key === '5') {
      return 'heading-5';
    }
    if (event.code === 'Digit6' || event.code === 'Numpad6' || key === '6') {
      return 'heading-6';
    }
    if (event.code === 'KeyC' || key === 'c') return 'code-block';
    return null;
  }
  if (event.shiftKey) {
    if (event.code === 'KeyS' || event.code === 'KeyX' || key === 's' || key === 'x') {
      return 'strike';
    }
    if (event.code === 'KeyB' || key === 'b') return 'blockquote';
    if (
      event.code === 'Digit7' ||
      event.code === 'Numpad7' ||
      key === '7' ||
      key === '&'
    ) {
      return 'ordered-list';
    }
    if (
      event.code === 'Digit8' ||
      event.code === 'Numpad8' ||
      key === '8' ||
      key === '*'
    ) {
      return 'bullet-list';
    }
    if (
      event.code === 'Digit9' ||
      event.code === 'Numpad9' ||
      key === '9' ||
      key === '('
    ) {
      return 'task-list';
    }
    return null;
  }
  if (event.code === 'KeyE' || key === 'e') return 'code';
  return null;
}
