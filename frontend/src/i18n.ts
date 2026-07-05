import { createI18n } from 'vue-i18n'

export type Lang = 'en' | 'fr' | 'ja'
export const LANGS: { code: Lang; label: string }[] = [
  { code: 'en', label: 'English' },
  { code: 'fr', label: 'Français' },
  { code: 'ja', label: '日本語' },
]

const STORAGE_KEY = 'mathesis.lang'

/** Chosen language, from localStorage or the browser, falling back to English. */
export function initialLang(): Lang {
  const saved = localStorage.getItem(STORAGE_KEY)
  if (saved === 'en' || saved === 'fr' || saved === 'ja') return saved
  const nav = (navigator.language || 'en').toLowerCase()
  if (nav.startsWith('fr')) return 'fr'
  if (nav.startsWith('ja')) return 'ja'
  return 'en'
}

export function saveLang(lang: Lang) {
  localStorage.setItem(STORAGE_KEY, lang)
}

// UI chrome + the getting-started guide. Per-function documentation lives in
// docs.ts (it is data, keyed by the same language codes).
const messages = {
  en: {
    tagline: 'exact mathematics, in your browser',
    nav: { docs: 'Docs', share: 'Share', source: 'source', home: 'Mathesis — start a fresh sheet', lang: 'Language', shareLine: 'Share this line', del: 'Delete this line' },
    hero: {
      eyebrow: 'exact by construction',
      approx: 'a calculator rounds to',
      caption: 'Every digit, exactly — no rounding, no server. Type an expression and press {enter}.',
    },
    composer: { placeholder: 'Type an expression…', stop: 'stop' },
    sol: { count: 'no solutions | {n} solution | {n} solutions', truncated: 'first {n} shown' },
    builder: {
      open: 'Solve builder',
      title: 'Solve builder',
      constraints: 'Constraints',
      constraintPh: 'e.g. x + y == 4',
      add: 'Add constraint',
      solveFor: 'Solve for',
      solveForPh: 'x, y',
      domain: 'Domain',
      preview: 'Preview',
      insert: 'Insert',
      cancel: 'Cancel',
    },
    toast: {
      copied: 'Link copied to clipboard',
      shared: 'Shared',
      failed: "Couldn't share — copy the address bar instead",
    },
    docs: {
      title: 'Documentation',
      search: 'Search functions…',
      close: 'Close',
      syntax: 'Syntax',
      examples: 'Examples',
      insert: 'Insert',
      guide: 'Getting started',
      empty: 'No functions match your search.',
    },
    version: 'engine',
    guide: [
      {
        h: 'What is Mathesis?',
        p: 'A computational notebook that runs entirely in your browser. Type an expression in a Wolfram-style syntax and get an exact answer, computed on your own machine — no server. The mathematics is provided by pure-Rust engines (puremp for exact arithmetic, z3rs for solving) compiled to WebAssembly.',
      },
      {
        h: 'Entering expressions',
        p: 'Use the operators + - * / ^ and postfix ! (factorial); group with parentheses. Call functions with square brackets, Head[arg, …], and write lists with braces, {a, b, c}. Assign a variable with =, e.g. x = RandomPrime[1000], then reuse x in later cells (== stays equality). % refers to the previous result. Text for the SMT solver goes in double quotes, "…".',
      },
      {
        h: 'Exact, with a decimal alongside',
        p: 'Integers and fractions stay exact (1/3 + 1/3 → 2/3), and irrational leaves are kept symbolic (Pi → π, Sqrt[2] → √2), each shown with a decimal approximation. Once an irrational enters an arithmetic, the result becomes an arbitrary-precision real. N[x, d] prints d digits.',
      },
      {
        h: 'Using the notebook',
        p: 'Press Enter to evaluate, Shift+Enter for a new line. Click a past input to reuse it. Long computations run in the background — press stop to interrupt. Share any result, or the whole notebook, from the buttons in the header.',
      },
    ],
  },

  fr: {
    tagline: 'des mathématiques exactes, dans votre navigateur',
    nav: { docs: 'Docs', share: 'Partager', source: 'source', home: 'Mathesis — nouvelle feuille', lang: 'Langue', shareLine: 'Partager cette ligne', del: 'Supprimer cette ligne' },
    hero: {
      eyebrow: 'exact par construction',
      approx: 'une calculatrice arrondit à',
      caption:
        'Chaque chiffre, exactement — sans arrondi, sans serveur. Saisissez une expression et appuyez sur {enter}.',
    },
    composer: { placeholder: 'Saisissez une expression…', stop: 'arrêter' },
    sol: { count: 'aucune solution | {n} solution | {n} solutions', truncated: '{n} premières affichées' },
    builder: {
      open: 'Assistant Solve',
      title: 'Assistant Solve',
      constraints: 'Contraintes',
      constraintPh: 'ex. x + y == 4',
      add: 'Ajouter une contrainte',
      solveFor: 'Résoudre pour',
      solveForPh: 'x, y',
      domain: 'Domaine',
      preview: 'Aperçu',
      insert: 'Insérer',
      cancel: 'Annuler',
    },
    toast: {
      copied: 'Lien copié dans le presse-papiers',
      shared: 'Partagé',
      failed: "Partage impossible — copiez la barre d'adresse",
    },
    docs: {
      title: 'Documentation',
      search: 'Rechercher une fonction…',
      close: 'Fermer',
      syntax: 'Syntaxe',
      examples: 'Exemples',
      insert: 'Insérer',
      guide: 'Prise en main',
      empty: 'Aucune fonction ne correspond.',
    },
    version: 'moteur',
    guide: [
      {
        h: 'Qu’est-ce que Mathesis ?',
        p: 'Un carnet de calcul qui fonctionne entièrement dans votre navigateur. Saisissez une expression dans une syntaxe à la Wolfram et obtenez une réponse exacte, calculée sur votre propre machine — sans serveur. Les mathématiques sont fournies par des moteurs en Rust pur (puremp pour l’arithmétique exacte, z3rs pour la résolution) compilés en WebAssembly.',
      },
      {
        h: 'Saisir des expressions',
        p: 'Utilisez les opérateurs + - * / ^ et le ! postfixé (factorielle) ; groupez avec des parenthèses. Appelez les fonctions avec des crochets, Head[arg, …], et écrivez les listes avec des accolades, {a, b, c}. Affectez une variable avec =, p. ex. x = RandomPrime[1000], puis réutilisez x dans les cellules suivantes (== reste l’égalité). % désigne le résultat précédent. Le texte destiné au solveur SMT se met entre guillemets doubles, "…".',
      },
      {
        h: 'Exact, avec la valeur décimale',
        p: 'Les entiers et les fractions restent exacts (1/3 + 1/3 → 2/3), et les irrationnels sont conservés sous forme symbolique (Pi → π, Sqrt[2] → √2), chacun accompagné d’une valeur décimale approchée. Dès qu’un irrationnel entre dans un calcul, le résultat devient un réel en précision arbitraire. N[x, d] affiche d chiffres.',
      },
      {
        h: 'Utiliser le carnet',
        p: 'Appuyez sur Entrée pour évaluer, Maj+Entrée pour un saut de ligne. Cliquez sur une entrée précédente pour la réutiliser. Les calculs longs s’exécutent en arrière-plan — appuyez sur « arrêter » pour interrompre. Partagez un résultat, ou tout le carnet, depuis les boutons de l’en-tête.',
      },
    ],
  },

  ja: {
    tagline: '正確な数学を、ブラウザーで',
    nav: { docs: 'ドキュメント', share: '共有', source: 'ソース', home: 'Mathesis — 新しいシート', lang: '言語', shareLine: 'この行を共有', del: 'この行を削除' },
    hero: {
      eyebrow: '厳密性を第一に',
      approx: '電卓が丸めると',
      caption: 'すべての桁を正確に — 丸めなし、サーバーなし。式を入力して {enter} を押してください。',
    },
    composer: { placeholder: '式を入力…', stop: '停止' },
    sol: { count: '解なし | {n} 個の解 | {n} 個の解', truncated: '先頭 {n} 件を表示' },
    builder: {
      open: 'Solve ビルダー',
      title: 'Solve ビルダー',
      constraints: '制約',
      constraintPh: '例: x + y == 4',
      add: '制約を追加',
      solveFor: '解く変数',
      solveForPh: 'x, y',
      domain: '領域',
      preview: 'プレビュー',
      insert: '挿入',
      cancel: 'キャンセル',
    },
    toast: {
      copied: 'リンクをクリップボードにコピーしました',
      shared: '共有しました',
      failed: '共有できませんでした — アドレスバーをコピーしてください',
    },
    docs: {
      title: 'ドキュメント',
      search: '関数を検索…',
      close: '閉じる',
      syntax: '構文',
      examples: '例',
      insert: '挿入',
      guide: 'はじめに',
      empty: '一致する関数がありません。',
    },
    version: 'エンジン',
    guide: [
      {
        h: 'Mathesis とは',
        p: 'ブラウザー内だけで動作する計算ノートブックです。Wolfram 風の構文で式を入力すると、サーバーを介さずお使いの端末で厳密な答えが得られます。数学の計算は、WebAssembly にコンパイルされた純 Rust エンジン（厳密計算の puremp、求解の z3rs）が担います。',
      },
      {
        h: '式の入力',
        p: '演算子 + - * / ^ と後置の !（階乗）を使い、括弧でまとめます。関数は角括弧 Head[引数, …] で呼び出し、リストは波括弧 {a, b, c} で書きます。= で変数に代入でき（例: x = RandomPrime[1000]）、以降のセルで x を再利用できます（== は等値のまま）。% は直前の結果を指します。SMT ソルバーに渡す文字列は二重引用符 "…" で囲みます。',
      },
      {
        h: '厳密な値と小数表示',
        p: '整数や分数は厳密なまま保たれ（1/3 + 1/3 → 2/3）、無理数は記号のまま扱われます（Pi → π、Sqrt[2] → √2）。いずれも小数の近似値が併記されます。計算に無理数が入ると、結果は任意精度の実数になります。N[x, d] は d 桁を表示します。',
      },
      {
        h: 'ノートブックの使い方',
        p: 'Enter で評価、Shift+Enter で改行します。過去の入力をクリックすると再利用できます。時間のかかる計算はバックグラウンドで実行され、「停止」で中断できます。ヘッダーのボタンから、結果やノートブック全体を共有できます。',
      },
    ],
  },
} as const

export const i18n = createI18n({
  legacy: false,
  globalInjection: true,
  locale: initialLang(),
  fallbackLocale: 'en',
  messages,
})
