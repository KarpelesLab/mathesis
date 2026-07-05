// Editor intelligence: an autocomplete source and a signature-help tooltip,
// both built from the documentation catalogue so they stay in sync with the
// engine's builtins and are localized.
import type { Completion, CompletionContext, CompletionResult, CompletionSource } from '@codemirror/autocomplete'
import { StateField } from '@codemirror/state'
import { showTooltip, type Tooltip } from '@codemirror/view'
import { CATEGORIES } from './docs'
import { i18n, type Lang } from './i18n'

// The argument part of a syntax, e.g. "Solve[c, vars]" → "[c, vars]", so a
// completion row doesn't repeat the function name (label + detail).
function argDetail(syntax: string): string | undefined {
  const b = syntax.indexOf('[')
  return b < 0 ? undefined : syntax.slice(b)
}

interface FnInfo {
  name: string
  syntax: string
  nullary: boolean
  desc: Record<Lang, string>
}

const tri = (en: string, fr: string, ja: string): Record<Lang, string> => ({ en, fr, ja })

// Language keywords not in the docs catalogue but worth completing.
const EXTRAS: FnInfo[] = [
  { name: 'True', syntax: 'True', nullary: true, desc: tri('Boolean true.', 'Booléen vrai.', '真。') },
  { name: 'False', syntax: 'False', nullary: true, desc: tri('Boolean false.', 'Booléen faux.', '偽。') },
  { name: 'Integers', syntax: 'Integers', nullary: true, desc: tri('The integer domain (for Solve/FindInstance).', 'Le domaine des entiers.', '整数の領域。') },
  { name: 'Reals', syntax: 'Reals', nullary: true, desc: tri('The real domain (for Solve/FindInstance).', 'Le domaine des réels.', '実数の領域。') },
  { name: 'And', syntax: 'And[a, b, …]', nullary: false, desc: tri('Logical conjunction (also a && b).', 'Conjonction logique (aussi a && b).', '論理積（a && b）。') },
  { name: 'Or', syntax: 'Or[a, b, …]', nullary: false, desc: tri('Logical disjunction (also a || b).', 'Disjonction logique (aussi a || b).', '論理和（a || b）。') },
  { name: 'Not', syntax: 'Not[a]', nullary: false, desc: tri('Logical negation.', 'Négation logique.', '否定。') },
  { name: 'Implies', syntax: 'Implies[a, b]', nullary: false, desc: tri('Logical implication a ⇒ b.', 'Implication a ⇒ b.', '含意 a ⇒ b。') },
  { name: 'Xor', syntax: 'Xor[a, b]', nullary: false, desc: tri('Exclusive or.', 'Ou exclusif.', '排他的論理和。') },
]

const FN_INFO: FnInfo[] = [
  ...CATEGORIES.flatMap((c) =>
    c.fns.map((f) => ({ name: f.name, syntax: f.syntax, nullary: c.id === 'constants', desc: f.desc })),
  ),
  ...EXTRAS,
]

const FN_MAP = new Map(FN_INFO.map((f) => [f.name, f]))

function esc(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
}

// Insert `Name[]` and place the cursor between the brackets.
function applyCall(view: import('@codemirror/view').EditorView, completion: Completion, from: number, to: number) {
  view.dispatch({
    changes: { from, to, insert: `${completion.label}[]` },
    selection: { anchor: from + completion.label.length + 1 },
  })
}

/** Completion source offering the builtins, localized. `openBuilder` is invoked
 *  when the user picks the visual Solve builder entry. */
type WithDesc = Completion & { desc?: string }

export function completionSource(getLang: () => Lang, openBuilder: () => void): CompletionSource {
  return (ctx: CompletionContext): CompletionResult | null => {
    const word = ctx.matchBefore(/[A-Za-z][A-Za-z0-9]*/)
    if (!word || (word.from === word.to && !ctx.explicit)) return null
    const lang = getLang()
    const options: WithDesc[] = FN_INFO.map((f) => ({
      label: f.name,
      type: f.nullary ? 'constant' : 'function',
      detail: f.nullary ? undefined : argDetail(f.syntax),
      desc: f.desc[lang],
      apply: f.nullary ? undefined : applyCall,
    }))
    // When the user is typing Solve, offer the visual builder.
    const typed = ctx.state.sliceDoc(word.from, word.to).toLowerCase()
    if ('solve'.startsWith(typed) || 'findinstance'.startsWith(typed)) {
      options.push({
        label: 'SolveBuilder',
        displayLabel: `⊞ ${i18n.global.t('builder.open')}`,
        type: 'keyword',
        desc: i18n.global.t('builder.title'),
        boost: 2,
        apply: () => openBuilder(),
      })
    }
    return { from: word.from, options, validFor: /^[A-Za-z][A-Za-z0-9]*$/ }
  }
}

/** Renders a completion's description as a second line under its name. Plugged
 *  in via `autocompletion({ addToOptions })`. */
export function descRenderer(completion: Completion): Node | null {
  const desc = (completion as WithDesc).desc
  if (!desc) return null
  const el = document.createElement('div')
  el.className = 'cm-completionDesc'
  el.textContent = desc
  return el
}

// --- signature help ---------------------------------------------------------

interface CallCtx {
  head: string
  argIndex: number
}

/** If the cursor sits inside a `Head[ … ]`, the enclosing head and 0-based
 *  argument index (counting top-level commas). */
function callContext(state: import('@codemirror/state').EditorState): CallCtx | null {
  const doc = state.doc.toString()
  const cur = state.selection.main.head

  // Find the innermost unclosed '[' before the cursor (ignoring {}, () for the
  // bracket search — nested calls use [ ]).
  let depth = 0
  let open = -1
  for (let i = cur - 1; i >= 0; i--) {
    const c = doc[i]
    if (c === ']') depth++
    else if (c === '[') {
      if (depth === 0) {
        open = i
        break
      }
      depth--
    }
  }
  if (open < 0) return null

  // The head identifier immediately before '['.
  let j = open
  while (j > 0 && /[A-Za-z0-9]/.test(doc[j - 1])) j--
  const head = doc.slice(j, open)
  if (!head || !/^[A-Za-z]/.test(head)) return null

  // Active argument = top-level commas between '[' and the cursor.
  let argIndex = 0
  let d = 0
  for (let i = open + 1; i < cur; i++) {
    const c = doc[i]
    if (c === '[' || c === '{' || c === '(') d++
    else if (c === ']' || c === '}' || c === ')') d--
    else if (c === ',' && d === 0) argIndex++
  }
  return { head, argIndex }
}

/** Render the signature tooltip HTML, bolding the active argument. */
function renderSig(info: FnInfo, argIndex: number, desc: string): string {
  const b = info.syntax.indexOf('[')
  const e = info.syntax.lastIndexOf(']')
  let sig: string
  if (b < 0 || e < b) {
    sig = esc(info.syntax)
  } else {
    const head = esc(info.syntax.slice(0, b))
    const inner = info.syntax.slice(b + 1, e)
    // Split by top-level commas.
    const parts: string[] = []
    let d = 0
    let last = 0
    for (let i = 0; i < inner.length; i++) {
      const ch = inner[i]
      if (ch === '[' || ch === '{' || ch === '(') d++
      else if (ch === ']' || ch === '}' || ch === ')') d--
      else if (ch === ',' && d === 0) {
        parts.push(inner.slice(last, i))
        last = i + 1
      }
    }
    parts.push(inner.slice(last))
    const rendered = parts
      .map((p, i) =>
        i === argIndex ? `<b class="sig-active">${esc(p.trim())}</b>` : esc(p.trim()),
      )
      .join(', ')
    sig = `${head}[${rendered}]`
  }
  return `<div class="sig-syntax">${sig}</div><div class="sig-desc">${esc(desc)}</div>`
}

/** A StateField that shows a signature tooltip when the cursor is inside a
 *  known function call. `openBuilder` is offered inside Solve/FindInstance. */
export function signatureField(getLang: () => Lang, openBuilder: () => void) {
  interface Keyed extends Tooltip {
    key?: string
  }
  return StateField.define<Keyed | null>({
    create: () => null,
    update(value, tr) {
      const ctx = callContext(tr.state)
      if (!ctx) return null
      const info = FN_MAP.get(ctx.head)
      if (!info || info.nullary) return null
      const key = `${ctx.head}:${ctx.argIndex}:${getLang()}`
      if (value && value.key === key) return value
      const buildable = info.name === 'Solve' || info.name === 'FindInstance'
      const tip: Keyed = {
        pos: tr.state.selection.main.head,
        above: true,
        arrow: false,
        key,
        create: () => {
          const dom = document.createElement('div')
          dom.className = 'cm-sig'
          dom.innerHTML = renderSig(info, ctx.argIndex, info.desc[getLang()])
          if (buildable) {
            const btn = document.createElement('button')
            btn.className = 'sig-builder'
            btn.textContent = `⊞ ${i18n.global.t('builder.open')}`
            // mousedown (not click) so the editor keeps focus and the tooltip
            // isn't dismissed before the handler runs.
            btn.addEventListener('mousedown', (e) => {
              e.preventDefault()
              openBuilder()
            })
            dom.appendChild(btn)
          }
          return { dom }
        },
      }
      return tip
    },
    provide: (f) => showTooltip.from(f),
  })
}
