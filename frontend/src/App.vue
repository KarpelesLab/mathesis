<script setup lang="ts">
import { nextTick, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import Editor from './components/Editor.vue'
import MathOutput from './components/MathOutput.vue'
import Graphics from './components/Graphics.vue'
import Solutions from './components/Solutions.vue'
import SolveBuilder from './components/SolveBuilder.vue'
import DocsPanel from './components/DocsPanel.vue'
import LangSwitch from './components/LangSwitch.vue'
import {
  cancelCurrent,
  CancelledError,
  engineVersion,
  evaluateInput,
  type EvalResult,
} from './engine'
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
  /** null while the computation is in flight. */
  result: EvalResult | null
  pending: boolean
  /** Set once a computation has been running long enough to warrant a notice. */
  slow?: boolean
}

/** How long a computation runs before we surface the "taking a while" notice. */
const SLOW_NOTICE_MS = 5000

const { t } = useI18n({ useScope: 'global' })

const entries = ref<Entry[]>([])
const counter = ref(0)
const version = ref('')
const busy = ref(false)
const showDocs = ref(false)
const showBuilder = ref(false)
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
  'Factor[360]',
  'N[Pi, 50]',
  'Plot[Sin[x], {x, 0, 2*Pi}]',
  '(1 + I)^2',
  'Sqrt[-4]',
  'Det[{{1, 2}, {3, 4}}]',
  'PowerMod[7, 100, 13]',
  'Fibonacci[100]',
  'Solve[x + y == 4 && x >= 0 && y >= 0, {x, y}]',
  'LatticeReduce[{{1, 1, 1}, {-1, 0, 2}, {3, 5, 6}}]',
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

// A result whose plain text is this long or longer is not fed to KaTeX — the
// DOM/typesetting cost of a multi-thousand-digit number would itself freeze the
// page. We show it as truncated monospace text instead.
const HUGE_OUTPUT = 4000

async function scrollToBottom() {
  await nextTick()
  scroller.value?.scrollTo({ top: scroller.value.scrollHeight, behavior: 'smooth' })
}

async function run(input: string) {
  if (busy.value) return
  busy.value = true

  // Show the input immediately as a pending cell, before we start computing,
  // so a long-running calculation is visible (and stoppable) as it runs.
  counter.value += 1
  const n = counter.value
  entries.value.push({ n, input, result: null, pending: true })
  historyPos.value = entries.value.length
  await scrollToBottom()

  // Resolve the cell by its stable id, not its index — it may have been deleted
  // (or reordered) while the computation was in flight.
  const settle = (result: EvalResult) => {
    const cell = entries.value.find((e) => e.n === n)
    if (cell) {
      cell.result = result
      cell.pending = false
    }
  }

  // No wall-clock limit — a long computation runs until it finishes or the user
  // stops it. After a few seconds we surface a notice pointing at Stop.
  const slowTimer = setTimeout(() => {
    const cell = entries.value.find((e) => e.n === n)
    if (cell?.pending) cell.slow = true
  }, SLOW_NOTICE_MS)

  try {
    settle(await evaluateInput(input))
  } catch (e) {
    const error = e instanceof CancelledError ? 'computation cancelled' : `engine error: ${String(e)}`
    settle({ ok: false, error })
  } finally {
    clearTimeout(slowTimer)
    busy.value = false
    await scrollToBottom()
  }
}

function stop() {
  cancelCurrent()
}

function deleteCell(entry: Entry) {
  const i = entries.value.findIndex((e) => e.n === entry.n)
  if (i >= 0) entries.value.splice(i, 1)
}

function isHuge(r: EvalResult | null): boolean {
  return Boolean(r && r.ok && r.text && r.text.length >= HUGE_OUTPUT)
}

function preview(text: string): string {
  return text.length > 2000 ? `${text.slice(0, 2000)}…` : text
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
  const key: Record<ShareOutcome, string> = {
    shared: 'toast.shared',
    copied: 'toast.copied',
    cancelled: '',
    failed: 'toast.failed',
  }
  if (!key[outcome]) return
  toast.value = t(key[outcome])
  clearTimeout(toastTimer)
  toastTimer = setTimeout(() => (toast.value = ''), 2400)
}

function insertExample(ex: string) {
  editor.value?.setText(ex)
  showDocs.value = false
}

function onBuilderInsert(expr: string) {
  editor.value?.setText(expr)
  showBuilder.value = false
}

async function shareCell(entry: Entry) {
  const url = singleShareUrl(entry.input)
  const preview = entry.result?.ok ? `${entry.input} = ${entry.result.text}` : entry.input
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
      <a class="brand" href="./" :title="t('nav.home')">
        <span class="mark">∴</span>
        <span class="wordmark">Mathesis</span>
      </a>
      <div class="actions">
        <button class="repo docs-btn" @click="showDocs = true">{{ t('nav.docs') }}</button>
        <button
          class="share-nb"
          :disabled="entries.length === 0"
          :title="t('nav.share')"
          @click="shareNotebook"
        >
          <svg viewBox="0 0 24 24" class="ico" aria-hidden="true">
            <path
              d="M18 8a3 3 0 1 0-2.83-4H15a3 3 0 0 0 .12 3.36L8.9 10.7a3 3 0 1 0 0 2.6l6.22 3.34A3 3 0 1 0 18 16a3 3 0 0 0-1.9.68L9.88 13.3a3 3 0 0 0 0-2.6l6.22-3.38A3 3 0 0 0 18 8Z"
            />
          </svg>
          {{ t('nav.share') }}
        </button>
        <LangSwitch />
        <a class="repo" href="https://github.com/KarpelesLab/mathesis" target="_blank" rel="noopener">
          {{ t('nav.source') }} ↗
        </a>
      </div>
    </header>

    <main ref="scroller" class="scroll">
      <section v-if="entries.length === 0" class="hero">
        <p class="eyebrow">{{ t('hero.eyebrow') }}</p>
        <div class="hero-demo">
          <div class="hero-expr">2<sup>128</sup></div>
          <p class="hero-approx">
            <span class="approx-label">{{ t('hero.approx') }}</span>3.4028 × 10³⁸
          </p>
          <p class="hero-exact">
            <span class="tf">∴</span>
            <span class="hero-num">340282366920938463463374607431768211456</span>
          </p>
        </div>
        <p class="hero-cap">
          <i18n-t keypath="hero.caption">
            <template #enter><kbd>Enter</kbd></template>
          </i18n-t>
        </p>
        <div class="examples">
          <button v-for="ex in examples" :key="ex" class="chip" @click="useExample(ex)">
            {{ ex }}
          </button>
        </div>
      </section>

      <ol class="cells">
        <li v-for="entry in entries" :key="entry.n" class="cell">
          <div class="cell-actions">
            <button class="cell-btn" :title="t('nav.shareLine')" @click="shareCell(entry)">
              <svg viewBox="0 0 24 24" class="ico" aria-hidden="true">
                <path
                  d="M18 8a3 3 0 1 0-2.83-4H15a3 3 0 0 0 .12 3.36L8.9 10.7a3 3 0 1 0 0 2.6l6.22 3.34A3 3 0 1 0 18 16a3 3 0 0 0-1.9.68L9.88 13.3a3 3 0 0 0 0-2.6l6.22-3.38A3 3 0 0 0 18 8Z"
                />
              </svg>
            </button>
            <button class="cell-btn cell-delete" :title="t('nav.del')" @click="deleteCell(entry)">
              <svg viewBox="0 0 24 24" class="ico" aria-hidden="true">
                <path
                  d="M9 3a1 1 0 0 0-1 1v1H4v2h16V5h-4V4a1 1 0 0 0-1-1H9Zm-3 6 .87 11.14A2 2 0 0 0 8.86 22h6.28a2 2 0 0 0 1.99-1.86L18 9H6Z"
                />
              </svg>
            </button>
          </div>
          <div class="io in">
            <span class="gutter idx">{{ entry.n }}</span>
            <button class="source" :title="'Reuse this input'" @click="reuse(entry.input)">
              {{ entry.input }}
            </button>
          </div>
          <div class="io out">
            <span class="gutter tf" :title="'Out[' + entry.n + ']'">∴</span>
            <div class="result">
              <span v-if="entry.pending" class="pending">
                <span class="dots" aria-label="computing"><i></i><i></i><i></i></span>
                <span v-if="entry.slow" class="slow-note">{{ t('composer.slow') }}</span>
                <button class="stop-inline" :title="t('composer.stop')" @click="stop">
                  {{ t('composer.stop') }}
                </button>
              </span>
              <template v-else-if="entry.result">
                <Graphics v-if="entry.result.graphics" :data="entry.result.graphics" />
                <Solutions v-else-if="entry.result.solutions" :data="entry.result.solutions" />
                <div v-else-if="isHuge(entry.result)" class="huge">
                  <code>{{ preview(entry.result.text!) }}</code>
                  <span class="huge-note">{{ entry.result.text!.length.toLocaleString() }} characters — truncated for display</span>
                </div>
                <pre v-else-if="entry.result.plain" class="plain-out">{{ entry.result.text }}</pre>
                <template v-else>
                  <MathOutput
                    v-if="entry.result.ok && entry.result.tex"
                    :tex="entry.result.tex"
                  />
                  <div v-else class="error">{{ entry.result.error }}</div>
                  <div v-if="entry.result.approx" class="approx">≈ {{ entry.result.approx }}</div>
                </template>
              </template>
            </div>
          </div>
        </li>
      </ol>
    </main>

    <footer class="composer">
      <div class="composer-inner">
        <span class="gutter tf live" :title="'In[' + (counter + 1) + ']'">∴</span>
        <Editor
          ref="editor"
          :placeholder="t('composer.placeholder')"
          @submit="run"
          @history="browseHistory"
          @open-builder="showBuilder = true"
        />
        <span class="hint">{{ busy ? '…' : '↵' }}</span>
      </div>
    </footer>

    <transition name="toast">
      <div v-if="toast" class="toast">{{ toast }}</div>
    </transition>

    <transition name="docs">
      <DocsPanel v-if="showDocs" @close="showDocs = false" @insert="insertExample" />
    </transition>

    <SolveBuilder v-if="showBuilder" @insert="onBuilderInsert" @close="showBuilder = false" />

    <div class="version" v-if="version">{{ t('version') }} v{{ version }}</div>
  </div>
</template>
