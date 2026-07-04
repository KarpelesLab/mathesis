/// <reference lib="webworker" />
//
// The engine runs here, off the main thread, so a heavy computation never
// freezes the UI. There is no cooperative cancellation inside wasm — the only
// way to stop a runaway computation is for the main thread to `terminate()`
// this worker, which is exactly what the timeout / Stop button do.
import init, { evaluate, version } from './pkg/mathesis.js'

const ctx = self as unknown as DedicatedWorkerGlobalScope

// Announce readiness (and the engine version) once wasm has instantiated.
const ready = init().then(() => {
  ctx.postMessage({ type: 'ready', version: version() })
})

ctx.onmessage = async (e: MessageEvent) => {
  const msg = e.data
  if (msg?.type !== 'eval') return

  await ready
  let result: unknown
  try {
    result = JSON.parse(evaluate(msg.input as string))
  } catch (err) {
    result = { ok: false, error: `engine error: ${String(err)}` }
  }
  ctx.postMessage({ type: 'result', id: msg.id, result })
}
