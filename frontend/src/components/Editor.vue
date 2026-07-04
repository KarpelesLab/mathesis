<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref } from 'vue'
import { EditorState } from '@codemirror/state'
import { EditorView, keymap, placeholder } from '@codemirror/view'
import {
  defaultKeymap,
  history,
  historyKeymap,
  insertNewlineAndIndent,
} from '@codemirror/commands'

const emit = defineEmits<{
  (e: 'submit', value: string): void
  (e: 'history', dir: -1 | 1): void
}>()

const host = ref<HTMLDivElement | null>(null)
let view: EditorView | null = null

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
        placeholder('Type an expression…'),
        keymap.of([
          // Enter evaluates; Shift-Enter inserts a newline for multi-line input.
          { key: 'Enter', run: submit },
          { key: 'Shift-Enter', run: insertNewlineAndIndent },
          {
            key: 'ArrowUp',
            run: (v) => {
              if (v.state.doc.lines > 1) return false
              emit('history', -1)
              return true
            },
          },
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
