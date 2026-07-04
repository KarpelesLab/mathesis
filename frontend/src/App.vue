<script setup lang="ts">
import { nextTick, onMounted, ref } from 'vue'
import Editor from './components/Editor.vue'
import MathOutput from './components/MathOutput.vue'
import { engineVersion, evaluateInput, type EvalResult } from './engine'
import {
  notebookShareUrl,
  parseShareUrl,
  shareLink,
  singleShareUrl,
  type ShareOutcome,
} from './share'

interface Entry {
  n: number
  input: string
  result: EvalResult
}

const entries = ref<Entry[]>([])
const counter = ref(0)
const version = ref('')
const busy = ref(false)
const editor = ref<InstanceType<typeof Editor> | null>(null)
const scroller = ref<HTMLElement | null>(null)
const toast = ref('')

// Index into `entries` while browsing input history with the arrow keys;
// entries.length means "at the live prompt".
const historyPos = ref(0)

let toastTimer: ReturnType<typeof setTimeout> | undefined

const examples = [
  '2^128',
  '1/3 + 1/3 + 1/3',
  '(1 + 1/2)^10',
  'Factor[360]',
  '20!',
  'GCD[462, 1071]',
  'Fibonacci[100]',
  'PrimeQ[2^61 - 1]',
  'Sqrt[152399025]',
  'N[22/7, 20]',
]

onMounted(async () => {
  try {
    version.value = await engineVersion()
  } catch {
    version.value = ''
  }
  // Replay a shared computation/notebook if the URL carries one.
  const shared = parseShareUrl()
  if (shared?.single) {
    await run(shared.single)
  } else if (shared?.notebook) {
    for (const input of shared.notebook) await run(input)
  }
})

async function run(input: string) {
  busy.value = true
  try {
    const result = await evaluateInput(input)
    counter.value += 1
    entries.value.push({ n: counter.value, input, result })
    historyPos.value = entries.value.length
    await nextTick()
    scroller.value?.scrollTo({ top: scroller.value.scrollHeight, behavior: 'smooth' })
  } catch (e) {
    counter.value += 1
    entries.value.push({
      n: counter.value,
      input,
      result: { ok: false, error: `engine failed to load: ${String(e)}` },
    })
  } finally {
    busy.value = false
  }
}

function browseHistory(dir: -1 | 1) {
  if (entries.value.length === 0) return
  let pos = historyPos.value + dir
  pos = Math.max(0, Math.min(entries.value.length, pos))
  historyPos.value = pos
  const text = pos === entries.value.length ? '' : entries.value[pos].input
  editor.value?.setText(text)
}

function useExample(ex: string) {
  editor.value?.setText(ex)
}

function reuse(input: string) {
  editor.value?.setText(input)
}

function flash(outcome: ShareOutcome) {
  const message: Record<ShareOutcome, string> = {
    shared: 'Shared',
    copied: 'Link copied to clipboard',
    cancelled: '',
    failed: "Couldn't share — copy the address bar instead",
  }
  const text = message[outcome]
  if (!text) return
  toast.value = text
  clearTimeout(toastTimer)
  toastTimer = setTimeout(() => (toast.value = ''), 2400)
}

async function shareCell(entry: Entry) {
  const url = singleShareUrl(entry.input)
  const preview = entry.result.ok ? `${entry.input} = ${entry.result.text}` : entry.input
  flash(await shareLink(url, 'Mathesis computation', preview))
}

async function shareNotebook() {
  if (entries.value.length === 0) return
  const url = notebookShareUrl(entries.value.map((e) => e.input))
  flash(await shareLink(url, 'Mathesis notebook', `A Mathesis notebook of ${entries.value.length} computation(s)`))
}
</script>

<template>
  <div class="app">
    <header class="topbar">
      <div class="brand">
        <span class="mark">∴</span>
        <div class="titles">
          <h1>Mathesis</h1>
          <p>a computational notebook that runs entirely in your browser</p>
        </div>
      </div>
      <div class="actions">
        <button
          class="share-nb"
          :disabled="entries.length === 0"
          title="Share this notebook as a link"
          @click="shareNotebook"
        >
          <svg viewBox="0 0 24 24" class="ico" aria-hidden="true">
            <path
              d="M18 8a3 3 0 1 0-2.83-4H15a3 3 0 0 0 .12 3.36L8.9 10.7a3 3 0 1 0 0 2.6l6.22 3.34A3 3 0 1 0 18 16a3 3 0 0 0-1.9.68L9.88 13.3a3 3 0 0 0 0-2.6l6.22-3.38A3 3 0 0 0 18 8Z"
            />
          </svg>
          Share
        </button>
        <a class="repo" href="https://github.com/KarpelesLab/mathesis" target="_blank" rel="noopener">
          source ↗
        </a>
      </div>
    </header>

    <main ref="scroller" class="scroll">
      <section v-if="entries.length === 0" class="welcome">
        <p class="lede">
          Type an expression and press <kbd>Enter</kbd>. Everything is computed exactly, on your
          machine — powered by pure-Rust arbitrary-precision arithmetic compiled to WebAssembly.
        </p>
        <div class="examples">
          <button v-for="ex in examples" :key="ex" class="chip" @click="useExample(ex)">
            {{ ex }}
          </button>
        </div>
      </section>

      <ol class="cells">
        <li v-for="entry in entries" :key="entry.n" class="cell">
          <button
            class="cell-share"
            title="Share this computation as a link"
            @click="shareCell(entry)"
          >
            <svg viewBox="0 0 24 24" class="ico" aria-hidden="true">
              <path
                d="M18 8a3 3 0 1 0-2.83-4H15a3 3 0 0 0 .12 3.36L8.9 10.7a3 3 0 1 0 0 2.6l6.22 3.34A3 3 0 1 0 18 16a3 3 0 0 0-1.9.68L9.88 13.3a3 3 0 0 0 0-2.6l6.22-3.38A3 3 0 0 0 18 8Z"
              />
            </svg>
          </button>
          <div class="io in">
            <span class="prompt in-prompt">In[{{ entry.n }}]</span>
            <button class="source" :title="'Reuse this input'" @click="reuse(entry.input)">
              {{ entry.input }}
            </button>
          </div>
          <div class="io out">
            <span class="prompt out-prompt">Out[{{ entry.n }}]</span>
            <div class="result">
              <MathOutput v-if="entry.result.ok && entry.result.tex" :tex="entry.result.tex" />
              <div v-else class="error">{{ entry.result.error }}</div>
            </div>
          </div>
        </li>
      </ol>
    </main>

    <footer class="composer">
      <div class="composer-inner">
        <span class="prompt live-prompt">In[{{ counter + 1 }}]</span>
        <Editor
          ref="editor"
          @submit="run"
          @history="browseHistory"
        />
        <span class="hint" :class="{ busy }">{{ busy ? '…' : '↵' }}</span>
      </div>
    </footer>

    <transition name="toast">
      <div v-if="toast" class="toast">{{ toast }}</div>
    </transition>

    <div class="version" v-if="version">engine v{{ version }}</div>
  </div>
</template>
