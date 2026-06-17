// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

// Shared "insert a {{variable}} into the config field the user is editing"
// machinery for the node config panel.
//
// Config inputs are plain controlled <input>/<textarea> scattered across the
// panel files (not a shared component), so rather than refactor hundreds of
// fields we (1) track the last-focused field + caret, and (2) insert text by
// driving React's own onChange: set the value via the native setter and
// dispatch a real `input` event, which makes the field's existing onChange ->
// set() run and keeps React state in sync.

export type EditableField = HTMLInputElement | HTMLTextAreaElement

export interface FieldSnapshot {
  el: EditableField
  value: string
  start: number
  end: number
}

let tracked: FieldSnapshot | null = null
type Listener = (snap: FieldSnapshot | null) => void
const listeners = new Set<Listener>()

/** A text-ish field we allow `{{...}}` insertion into (not number/checkbox/select). */
export function isInsertableField(el: EventTarget | null): el is EditableField {
  if (el instanceof HTMLTextAreaElement) return true
  if (el instanceof HTMLInputElement) {
    return ['text', 'url', 'search', 'email', 'tel', ''].includes(el.type)
  }
  return false
}

function snapshot(el: EditableField): FieldSnapshot {
  const len = el.value.length
  return {
    el,
    value: el.value,
    start: el.selectionStart ?? len,
    end: el.selectionEnd ?? len,
  }
}

/** Record the field (and current caret/selection) the user is interacting with. */
export function captureField(el: EditableField): void {
  tracked = snapshot(el)
  for (const l of listeners) l(tracked)
}

/** Current tracked field, if it is still in the DOM. */
export function currentField(): FieldSnapshot | null {
  if (tracked && document.body.contains(tracked.el)) return tracked
  return null
}

export function subscribe(l: Listener): () => void {
  listeners.add(l)
  return () => listeners.delete(l)
}

function setNativeValue(el: EditableField, value: string): void {
  const proto =
    el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype
  const setter = Object.getOwnPropertyDescriptor(proto, 'value')?.set
  setter?.call(el, value)
  el.dispatchEvent(new Event('input', { bubbles: true }))
}

/**
 * Replace the text in [start, end) of the tracked field with `text`.
 * Defaults to the last-known caret/selection. Returns false if no live field.
 */
export function replaceRange(text: string, start?: number, end?: number): boolean {
  const f = currentField()
  if (!f) return false
  const el = f.el
  const v = el.value
  const s = start ?? f.start
  const e = end ?? f.end
  const next = v.slice(0, s) + text + v.slice(e)
  setNativeValue(el, next)
  const pos = s + text.length
  el.focus()
  try {
    el.setSelectionRange(pos, pos)
  } catch {
    /* number-like inputs reject setSelectionRange */
  }
  tracked = snapshot(el)
  for (const l of listeners) l(tracked)
  return true
}

/** Insert `text` at the tracked caret (replacing any selection). */
export function insertText(text: string): boolean {
  return replaceRange(text)
}
