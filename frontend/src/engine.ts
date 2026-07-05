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
  /** Decimal approximation to show alongside an exact result (π, √2, a fraction). */
  approx?: string
  /** When set, `text` is opaque output (string / SMT) — render as monospace, not math. */
  plain?: boolean
  /** A Plot/Plot3D payload to draw (present for graphics results). */
  graphics?: Graphics
  /** A Solve result rendered as a table (variables × solutions). */
  solutions?: Solutions
  /** Human-readable message (present when not ok). */
  error?: string
}

export interface Solutions {
  vars: string[]
  /** One row per solution; each cell is a TeX string aligned to `vars`. */
  rows: string[][]
  count: number
  truncated: boolean
}

export type Graphics =
  | { kind: 'plot2d'; var: string; xmin: number; xmax: number; series: { points: [number, number | null][] }[] }
  | { kind: 'plot3d'; xmin: number; xmax: number; ymin: number; ymax: number; z: (number | null)[][] }

export class CancelledError extends Error {
  constructor() {
    super('computation cancelled')
    this.name = 'CancelledError'
  }
}

interface Pending {
  id: number
  resolve: (r: EvalResult) => void
  reject: (e: Error) => void
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
    const p = pending
    pending = null
    p.reject(err)
  }
}

/** Evaluate one line of input. The computation runs until it finishes, the
 *  worker crashes, or the caller cancels it via `cancelCurrent` — there is no
 *  wall-clock limit. */
export function evaluateInput(input: string): Promise<EvalResult> {
  if (pending) {
    return Promise.reject(new Error('a computation is already running'))
  }
  const w = ensureWorker()
  const id = ++seq

  return new Promise<EvalResult>((resolve, reject) => {
    pending = { id, resolve, reject }
    w.postMessage({ type: 'eval', id, input })
  })
}

/** Stop the current computation, if any. Returns whether something was cancelled. */
export function cancelCurrent(): boolean {
  if (!pending) return false
  const p = pending
  pending = null
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
