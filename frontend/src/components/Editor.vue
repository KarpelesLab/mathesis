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
        signatureField(getLang),
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
