<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { EditorState, Compartment } from '@codemirror/state'
import { EditorView, keymap, placeholder } from '@codemirror/view'
import {
  defaultKeymap,
  history,
  historyKeymap,
  insertNewlineAndIndent,
} from '@codemirror/commands'
import {
  autocompletion,
  acceptCompletion,
  closeCompletion,
  moveCompletionSelection,
} from '@codemirror/autocomplete'
import { completionSource, signatureField } from '../editor-assist'
import type { Lang } from '../i18n'

const { locale } = useI18n({ useScope: 'global' })
const getLang = () => locale.value as Lang

const props = defineProps<{ placeholder?: string }>()
const emit = defineEmits<{
  (e: 'submit', value: string): void
  (e: 'history', dir: -1 | 1): void
  (e: 'openBuilder'): void
}>()

const host = ref<HTMLDivElement | null>(null)
let view: EditorView | null = null
// Placeholder lives in its own compartment so it can be swapped when the UI
// language changes.
const placeholderComp = new Compartment()

// A restrained editor theme that inherits the page's typography and colors so
// the input line feels native to the notebook rather than bolted on.
const theme = EditorView.theme({
  '&': {
    fontSize: '1.05rem',
    color: 'var(--chalk)',
    backgroundColor: 'transparent',
  },
  '.cm-content': {
    fontFamily: 'var(--font-mono)',
    caretColor: 'var(--amber)',
    padding: '0.15rem 0',
  },
  '.cm-line': { padding: '0' },
  '&.cm-focused': { outline: 'none' },
  '.cm-cursor': { borderLeftColor: 'var(--amber)', borderLeftWidth: '2px' },
  '.cm-placeholder': { color: 'var(--dust)', fontStyle: 'normal' },
  '.cm-selectionBackground, ::selection': {
    backgroundColor: 'var(--amber-soft)',
  },
  // Autocomplete popup + signature tooltip. These must live in the editor theme
  // (not global CSS) to override CodeMirror's base theme, which is injected into
  // <head> after our stylesheet.
  '.cm-tooltip': {
    background: 'var(--slate-hi)',
    border: '1px solid var(--rule)',
    borderRadius: '8px',
    color: 'var(--chalk)',
    boxShadow: '0 10px 34px rgba(0, 0, 0, 0.5)',
  },
  '.cm-tooltip.cm-tooltip-autocomplete > ul': {
    fontFamily: 'var(--font-mono)',
    fontSize: '0.9rem',
    maxHeight: '15rem',
  },
  '.cm-tooltip-autocomplete ul li': {
    padding: '0.28rem 0.65rem',
    color: 'var(--chalk-dim)',
  },
  '.cm-tooltip-autocomplete ul li[aria-selected]': {
    background: 'var(--amber-soft)',
    color: 'var(--chalk)',
  },
  '.cm-completionLabel': { color: 'var(--chalk)' },
  '.cm-completionDetail': {
    marginLeft: '0.6rem',
    color: 'var(--faint)',
    fontStyle: 'normal',
    fontSize: '0.85em',
  },
  '.cm-completionMatchedText': {
    color: 'var(--amber)',
    textDecoration: 'none',
    fontWeight: '600',
  },
  '.cm-completionInfo': {
    margin: '0 0.4rem',
    padding: '0.5rem 0.7rem',
    maxWidth: '20rem',
    background: 'var(--slate-hi)',
    border: '1px solid var(--rule)',
    borderRadius: '8px',
    color: 'var(--chalk-dim)',
    fontFamily: 'var(--font-serif)',
    fontSize: '0.9rem',
    lineHeight: '1.5',
  },
  '.cm-sig': { padding: '0.45rem 0.65rem', maxWidth: '24rem' },
  '.sig-syntax': { fontFamily: 'var(--font-mono)', fontSize: '0.9rem', color: 'var(--chalk)' },
  '.sig-active': { color: 'var(--amber)' },
  '.sig-desc': {
    marginTop: '0.25rem',
    fontFamily: 'var(--font-serif)',
    fontSize: '0.85rem',
    color: 'var(--dust)',
  },
  '.sig-builder': {
    display: 'block',
    marginTop: '0.5rem',
    fontFamily: 'var(--font-mono)',
    fontSize: '0.8rem',
    color: 'var(--amber)',
    background: 'var(--amber-soft)',
    border: 'none',
    borderRadius: '6px',
    padding: '0.3rem 0.6rem',
    cursor: 'pointer',
  },
})

function currentText(): string {
  return view?.state.doc.toString() ?? ''
}

function reset() {
  if (!view) return
  view.dispatch({
    changes: { from: 0, to: view.state.doc.length, insert: '' },
  })
}

function submit(): boolean {
  const text = currentText().trim()
  if (text.length === 0) return true
  emit('submit', text)
  reset()
  return true
}

onMounted(() => {
  view = new EditorView({
    parent: host.value!,
    state: EditorState.create({
      doc: '',
      extensions: [
        history(),
        placeholderComp.of(placeholder(props.placeholder ?? '')),
        autocompletion({
          override: [completionSource(getLang, () => emit('openBuilder'))],
          defaultKeymap: false,
          activateOnTyping: true,
          icons: false,
        }),
        signatureField(getLang, () => emit('openBuilder')),
        keymap.of([
          // When the completion popup is open these accept/navigate it; when it
          // is closed they fall through (the commands return false) to submit
          // and input-history behaviour.
          { key: 'Enter', run: acceptCompletion },
          { key: 'Enter', run: submit },
          { key: 'Shift-Enter', run: insertNewlineAndIndent },
          { key: 'Escape', run: closeCompletion },
          { key: 'ArrowUp', run: moveCompletionSelection(false) },
          {
            key: 'ArrowUp',
            run: (v) => {
              if (v.state.doc.lines > 1) return false
              emit('history', -1)
              return true
            },
          },
          { key: 'ArrowDown', run: moveCompletionSelection(true) },
          {
            key: 'ArrowDown',
            run: (v) => {
              if (v.state.doc.lines > 1) return false
              emit('history', 1)
              return true
            },
          },
          ...historyKeymap,
          ...defaultKeymap,
        ]),
        theme,
        EditorView.lineWrapping,
      ],
    }),
  })
  view.focus()
})

watch(
  () => props.placeholder,
  (v) => {
    view?.dispatch({ effects: placeholderComp.reconfigure(placeholder(v ?? '')) })
  },
)

onBeforeUnmount(() => {
  view?.destroy()
  view = null
})

function setText(text: string) {
  if (!view) return
  view.dispatch({
    changes: { from: 0, to: view.state.doc.length, insert: text },
    selection: { anchor: text.length },
  })
  view.focus()
}

defineExpose({
  focus: () => view?.focus(),
  setText,
})
</script>

<template>
  <div ref="host" class="editor"></div>
</template>

<style scoped>
.editor {
  flex: 1;
  min-width: 0;
}
</style>
