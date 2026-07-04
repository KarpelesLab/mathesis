<script setup lang="ts">
import { nextTick, onMounted, ref } from 'vue'
import Editor from './components/Editor.vue'
import MathOutput from './components/MathOutput.vue'
import { engineVersion, evaluateInput, type EvalResult } from './engine'

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

// Index into `entries` while browsing input history with the arrow keys;
// entries.length means "at the live prompt".
const historyPos = ref(0)

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
      <a class="repo" href="https://github.com/KarpelesLab/mathesis" target="_blank" rel="noopener">
        source ↗
      </a>
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

    <div class="version" v-if="version">engine v{{ version }}</div>
  </div>
</template>
