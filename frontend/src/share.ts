// Sharing: encode a computation (or a whole notebook) into a URL that
// reconstructs it on open. Everything lives in the URL hash, so sharing is
// fully client-side — no server, and it never hits the Pages 404 handler.
//
//   #c=<base64url(expr)>              a single computation
//   #n=<base64url(JSON string[])>     an ordered notebook of inputs

export interface Shared {
  single?: string
  notebook?: string[]
}

export type ShareOutcome = 'shared' | 'copied' | 'cancelled' | 'failed'

// --- base64url over UTF-8 ---------------------------------------------------

function toB64Url(s: string): string {
  const bytes = new TextEncoder().encode(s)
  let bin = ''
  for (const b of bytes) bin += String.fromCharCode(b)
  return btoa(bin).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
}

function fromB64Url(s: string): string {
  const b64 = s.replace(/-/g, '+').replace(/_/g, '/')
  const bin = atob(b64)
  const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0))
  return new TextDecoder().decode(bytes)
}

// --- build & parse ----------------------------------------------------------

function baseUrl(): string {
  return window.location.origin + window.location.pathname
}

export function singleShareUrl(expr: string): string {
  return `${baseUrl()}#c=${toB64Url(expr)}`
}

export function notebookShareUrl(inputs: string[]): string {
  return `${baseUrl()}#n=${toB64Url(JSON.stringify(inputs))}`
}

/** Read a shared computation/notebook from the current URL hash, if any. */
export function parseShareUrl(): Shared | null {
  const hash = window.location.hash.replace(/^#/, '')
  if (!hash) return null
  const params = new URLSearchParams(hash)

  const c = params.get('c')
  if (c) {
    try {
      return { single: fromB64Url(c) }
    } catch {
      return null
    }
  }

  const n = params.get('n')
  if (n) {
    try {
      const parsed = JSON.parse(fromB64Url(n))
      if (Array.isArray(parsed) && parsed.every((x) => typeof x === 'string')) {
        return { notebook: parsed as string[] }
      }
    } catch {
      return null
    }
  }

  return null
}

// --- share action -----------------------------------------------------------

/**
 * Share `url` via the Web Share API when available, otherwise copy it to the
 * clipboard (with a legacy `execCommand` fallback). Returns what happened so the
 * caller can give appropriate feedback.
 */
export async function shareLink(url: string, title: string, text?: string): Promise<ShareOutcome> {
  if (typeof navigator !== 'undefined' && typeof navigator.share === 'function') {
    try {
      await navigator.share({ title, text, url })
      return 'shared'
    } catch (e) {
      // The user dismissing the native sheet is not a failure.
      if (e instanceof DOMException && e.name === 'AbortError') return 'cancelled'
      // Anything else (e.g. NotAllowedError on desktop) → fall back to copy.
    }
  }
  return copyToClipboard(url)
}

async function copyToClipboard(text: string): Promise<ShareOutcome> {
  if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text)
      return 'copied'
    } catch {
      /* fall through to legacy path */
    }
  }
  try {
    const ta = document.createElement('textarea')
    ta.value = text
    ta.setAttribute('readonly', '')
    ta.style.position = 'fixed'
    ta.style.opacity = '0'
    document.body.appendChild(ta)
    ta.select()
    const ok = document.execCommand('copy')
    document.body.removeChild(ta)
    return ok ? 'copied' : 'failed'
  } catch {
    return 'failed'
  }
}
