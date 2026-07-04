// Main-thread client for the Mathesis engine, which runs in a Web Worker (see
// worker.ts). This keeps the UI responsive and — crucially — makes computations
// interruptible: a runaway calculation is stopped by terminating the worker,
// then a fresh one is spawned for the next request.

export interface EvalResult {
  ok: boolean
  /** Plain-text form of the result (present when ok). */
  text?: string
  /** TeX form for KaTeX (present when ok). */
  tex?: string
  /** Human-readable message (present when not ok). */
  error?: string
}

export class TimeoutError extends Error {
  constructor(public readonly ms: number) {
    super(`computation exceeded ${ms} ms`)
    this.name = 'TimeoutError'
  }
}

export class CancelledError extends Error {
  constructor() {
    super('computation cancelled')
    this.name = 'CancelledError'
  }
}

/** Default wall-clock budget before a computation is force-stopped. */
export const DEFAULT_TIMEOUT_MS = 6000

interface Pending {
  id: number
  resolve: (r: EvalResult) => void
  reject: (e: Error) => void
  timer: ReturnType<typeof setTimeout>
}

let worker: Worker | null = null
let seq = 0
// Computations are serial (the notebook runs one at a time), so a single
// in-flight slot is all we need.
let pending: Pending | null = null
let cachedVersion: string | null = null
let versionWaiters: ((v: string) => void)[] = []

function spawn(): Worker {
  const w = new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' })

  w.onmessage = (e: MessageEvent) => {
    const msg = e.data
    if (msg?.type === 'ready') {
      cachedVersion = msg.version
      versionWaiters.forEach((r) => r(msg.version))
      versionWaiters = []
    } else if (msg?.type === 'result' && pending && pending.id === msg.id) {
      clearTimeout(pending.timer)
      const p = pending
      pending = null
      p.resolve(msg.result as EvalResult)
    }
  }

  w.onerror = () => {
    // A worker-level failure — most likely the engine exhausting memory on a
    // runaway computation. Reject the in-flight request and rebuild clean.
    failPending(new Error('the engine ran out of memory or crashed on this computation'))
    rebuild()
  }

  return w
}

function ensureWorker(): Worker {
  if (!worker) worker = spawn()
  return worker
}

/** Terminate the current worker (freeing its memory); it respawns on next use. */
function rebuild() {
  if (worker) {
    worker.terminate()
    worker = null
  }
}

function failPending(err: Error) {
  if (pending) {
    clearTimeout(pending.timer)
    const p = pending
    pending = null
    p.reject(err)
  }
}

/** Evaluate one line of input, rejecting with `TimeoutError` if it runs too long. */
export function evaluateInput(input: string, timeoutMs = DEFAULT_TIMEOUT_MS): Promise<EvalResult> {
  if (pending) {
    return Promise.reject(new Error('a computation is already running'))
  }
  const w = ensureWorker()
  const id = ++seq

  return new Promise<EvalResult>((resolve, reject) => {
    const timer = setTimeout(() => {
      if (pending && pending.id === id) {
        const p = pending
        pending = null
        p.reject(new TimeoutError(timeoutMs))
      }
      // Kill the (still-busy) worker so it stops computing and frees memory.
      rebuild()
    }, timeoutMs)

    pending = { id, resolve, reject, timer }
    w.postMessage({ type: 'eval', id, input })
  })
}

/** Stop the current computation, if any. Returns whether something was cancelled. */
export function cancelCurrent(): boolean {
  if (!pending) return false
  const p = pending
  pending = null
  clearTimeout(p.timer)
  rebuild()
  p.reject(new CancelledError())
  return true
}

/** The engine's crate version (resolved once the worker has loaded wasm). */
export function engineVersion(): Promise<string> {
  if (cachedVersion !== null) return Promise.resolve(cachedVersion)
  ensureWorker()
  return new Promise((resolve) => versionWaiters.push(resolve))
}
