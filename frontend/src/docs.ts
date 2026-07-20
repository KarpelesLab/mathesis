// The documentation catalogue: every builtin, grouped by category, with syntax,
// runnable examples, and a one-line description per language. Descriptions are
// data (keyed by the same language codes as i18n.ts).
import type { Lang } from './i18n'

export interface FnDoc {
  name: string
  syntax: string
  examples: string[]
  desc: Record<Lang, string>
}
export interface Category {
  id: string
  title: Record<Lang, string>
  fns: FnDoc[]
}

const t = (en: string, fr: string, ja: string): Record<Lang, string> => ({ en, fr, ja })

export const CATEGORIES: Category[] = [
  {
    id: 'constants',
    title: t('Constants', 'Constantes', '定数'),
    fns: [
      { name: 'Pi', syntax: 'Pi', examples: ['Pi', 'N[Pi, 50]'], desc: t('The constant π ≈ 3.14159.', 'La constante π ≈ 3,14159.', '円周率 π ≈ 3.14159。') },
      { name: 'E', syntax: 'E', examples: ['E', 'N[E, 40]'], desc: t("Euler's number e ≈ 2.71828.", "Le nombre d'Euler e ≈ 2,71828.", '自然対数の底 e ≈ 2.71828。') },
      { name: 'EulerGamma', syntax: 'EulerGamma', examples: ['N[EulerGamma, 30]'], desc: t('The Euler–Mascheroni constant γ ≈ 0.57722.', "La constante d'Euler–Mascheroni γ ≈ 0,57722.", 'オイラー・マスケローニ定数 γ ≈ 0.57722。') },
      { name: 'Catalan', syntax: 'Catalan', examples: ['N[Catalan, 30]'], desc: t("Catalan's constant ≈ 0.91597.", 'La constante de Catalan ≈ 0,91597.', 'カタラン定数 ≈ 0.91597。') },
      { name: 'I', syntax: 'I', examples: ['I', 'I^2', '(1 + I)^2'], desc: t('The imaginary unit, I² = −1.', "L'unité imaginaire, I² = −1.", '虚数単位。I² = −1。') },
    ],
  },
  {
    id: 'arithmetic',
    title: t('Arithmetic & rounding', 'Arithmétique et arrondis', '算術・丸め'),
    fns: [
      { name: 'Power', syntax: 'Power[a, b]  ·  a^b', examples: ['2^128', 'Power[3, 4]'], desc: t('a raised to the power b.', 'a élevé à la puissance b.', 'a の b 乗。') },
      { name: 'Sqrt', syntax: 'Sqrt[x]', examples: ['Sqrt[16]', 'Sqrt[2]', 'Sqrt[-4]'], desc: t('Square root — exact for perfect squares, symbolic (√n) or complex otherwise.', 'Racine carrée — exacte pour les carrés parfaits, symbolique (√n) ou complexe sinon.', '平方根。完全平方は厳密、その他は記号（√n）または複素数。') },
      { name: 'Abs', syntax: 'Abs[x]', examples: ['Abs[-5]', 'Abs[3 + 4*I]'], desc: t('Absolute value, or the modulus of a complex number.', "Valeur absolue, ou module d'un nombre complexe.", '絶対値、または複素数の大きさ。') },
      { name: 'Sign', syntax: 'Sign[x]', examples: ['Sign[-3]', 'Sign[0]'], desc: t('The sign: −1, 0, or 1.', 'Le signe : −1, 0 ou 1.', '符号（−1, 0, 1）。') },
      { name: 'N', syntax: 'N[x]  ·  N[x, d]', examples: ['N[Pi, 50]', 'N[1/7, 20]'], desc: t('Decimal approximation, with d digits if given.', 'Approximation décimale, avec d chiffres le cas échéant.', '小数近似。d を指定すると d 桁。') },
      { name: 'Floor', syntax: 'Floor[x]', examples: ['Floor[7/2]', 'Floor[Pi]'], desc: t('Greatest integer ≤ x.', 'Plus grand entier ≤ x.', 'x 以下で最大の整数（床関数）。') },
      { name: 'Ceiling', syntax: 'Ceiling[x]', examples: ['Ceiling[7/2]'], desc: t('Least integer ≥ x.', 'Plus petit entier ≥ x.', 'x 以上で最小の整数（天井関数）。') },
      { name: 'Round', syntax: 'Round[x]', examples: ['Round[7/2]', 'Round[Pi]'], desc: t('Nearest integer.', 'Entier le plus proche.', '最も近い整数（四捨五入）。') },
      { name: 'IntegerPart', syntax: 'IntegerPart[x]', examples: ['IntegerPart[7/2]'], desc: t('Integer part (truncation toward zero).', 'Partie entière (troncature vers zéro).', '整数部分（ゼロ方向へ切り捨て）。') },
      { name: 'FractionalPart', syntax: 'FractionalPart[x]', examples: ['FractionalPart[7/2]'], desc: t('Fractional part, x − IntegerPart[x].', 'Partie fractionnaire, x − IntegerPart[x].', '小数部分（x − IntegerPart[x]）。') },
    ],
  },
  {
    id: 'numbertheory',
    title: t('Number theory', 'Théorie des nombres', '整数論'),
    fns: [
      { name: 'Factor', syntax: 'Factor[n]', examples: ['Factor[360]'], desc: t('Prime factorization.', 'Décomposition en facteurs premiers.', '素因数分解。') },
      { name: 'Divisors', syntax: 'Divisors[n]', examples: ['Divisors[12]'], desc: t('All positive divisors.', 'Tous les diviseurs positifs.', '正の約数の一覧。') },
      { name: 'DivisorSigma', syntax: 'DivisorSigma[k, n]', examples: ['DivisorSigma[1, 12]', 'DivisorSigma[0, 12]'], desc: t('Sum of the k-th powers of the divisors (k = 0 counts them).', 'Somme des puissances k-ièmes des diviseurs (k = 0 les compte).', '約数の k 乗の和（k = 0 は約数の個数）。') },
      { name: 'EulerPhi', syntax: 'EulerPhi[n]', examples: ['EulerPhi[36]'], desc: t("Euler's totient: integers ≤ n coprime to n.", "Indicatrice d'Euler : entiers ≤ n premiers avec n.", 'オイラーのトーシェント関数：n 以下で n と互いに素な数の個数。') },
      { name: 'MoebiusMu', syntax: 'MoebiusMu[n]', examples: ['MoebiusMu[30]', 'MoebiusMu[4]'], desc: t('The Möbius function μ(n).', 'La fonction de Möbius μ(n).', 'メビウス関数 μ(n)。') },
      { name: 'Radical', syntax: 'Radical[n]', examples: ['Radical[360]'], desc: t('Product of the distinct prime factors.', 'Produit des facteurs premiers distincts.', '相異なる素因数の積（根基）。') },
      { name: 'GCD', syntax: 'GCD[a, b, …]', examples: ['GCD[462, 1071]'], desc: t('Greatest common divisor.', 'Plus grand commun diviseur (PGCD).', '最大公約数。') },
      { name: 'LCM', syntax: 'LCM[a, b, …]', examples: ['LCM[4, 6, 9]'], desc: t('Least common multiple.', 'Plus petit commun multiple (PPCM).', '最小公倍数。') },
      { name: 'PrimeQ', syntax: 'PrimeQ[n]', examples: ['PrimeQ[97]', 'PrimeQ[2^61 - 1]'], desc: t('True if n is prime.', 'Vrai si n est premier.', 'n が素数なら True。') },
      { name: 'NextPrime', syntax: 'NextPrime[n]', examples: ['NextPrime[100]'], desc: t('Smallest prime greater than n.', 'Plus petit premier supérieur à n.', 'n より大きい最小の素数。') },
      { name: 'PreviousPrime', syntax: 'PreviousPrime[n]', examples: ['PreviousPrime[100]'], desc: t('Largest prime less than n.', 'Plus grand premier inférieur à n.', 'n より小さい最大の素数。') },
      { name: 'Factorial', syntax: 'Factorial[n]  ·  n!', examples: ['20!', 'Factorial[10]'], desc: t('n! = 1·2···n.', 'n! = 1·2···n.', '階乗 n! = 1·2···n。') },
      { name: 'Binomial', syntax: 'Binomial[n, k]', examples: ['Binomial[49, 6]'], desc: t('Binomial coefficient C(n, k).', 'Coefficient binomial C(n, k).', '二項係数 C(n, k)。') },
      { name: 'Multinomial', syntax: 'Multinomial[k1, k2, …]', examples: ['Multinomial[1, 2, 3]'], desc: t('Multinomial coefficient.', 'Coefficient multinomial.', '多項係数。') },
      { name: 'Fibonacci', syntax: 'Fibonacci[n]', examples: ['Fibonacci[100]'], desc: t('The n-th Fibonacci number.', 'Le n-ième nombre de Fibonacci.', 'n 番目のフィボナッチ数。') },
      { name: 'LucasL', syntax: 'LucasL[n]', examples: ['LucasL[20]'], desc: t('The n-th Lucas number.', 'Le n-ième nombre de Lucas.', 'n 番目のリュカ数。') },
      { name: 'Mod', syntax: 'Mod[a, m]', examples: ['Mod[17, 5]', 'Mod[-3, 5]'], desc: t('Remainder of a modulo m (non-negative).', 'Reste de a modulo m (positif).', 'a を m で割った剰余（非負）。') },
      { name: 'Quotient', syntax: 'Quotient[a, b]', examples: ['Quotient[17, 5]'], desc: t('Integer quotient (floored division).', 'Quotient entier (division par défaut).', '整数商（床除算）。') },
      { name: 'PowerMod', syntax: 'PowerMod[a, b, m]', examples: ['PowerMod[7, 100, 13]', 'PowerMod[3, -1, 7]'], desc: t('a^b mod m; a negative b uses the modular inverse.', "a^b mod m ; un b négatif utilise l'inverse modulaire.", 'a^b mod m。b が負なら逆元を用いる。') },
      { name: 'ModularInverse', syntax: 'ModularInverse[a, m]', examples: ['ModularInverse[3, 7]'], desc: t('Inverse of a modulo m.', 'Inverse de a modulo m.', 'a の m を法とする逆元。') },
      { name: 'ExtendedGCD', syntax: 'ExtendedGCD[a, b]', examples: ['ExtendedGCD[12, 18]'], desc: t('{g, {s, t}} with g = gcd = s·a + t·b.', '{g, {s, t}} avec g = pgcd = s·a + t·b.', '{g, {s, t}}（g = 最大公約数 = s·a + t·b）。') },
      { name: 'JacobiSymbol', syntax: 'JacobiSymbol[a, n]', examples: ['JacobiSymbol[2, 15]'], desc: t('The Jacobi symbol (a/n).', 'Le symbole de Jacobi (a/n).', 'ヤコビ記号 (a/n)。') },
      { name: 'ChineseRemainder', syntax: 'ChineseRemainder[{r…}, {m…}]', examples: ['ChineseRemainder[{2, 3}, {3, 5}]'], desc: t('Smallest x with x ≡ rᵢ (mod mᵢ).', 'Plus petit x tel que x ≡ rᵢ (mod mᵢ).', 'x ≡ rᵢ (mod mᵢ) を満たす最小の x。') },
      { name: 'SqrtMod', syntax: 'SqrtMod[a, p]', examples: ['SqrtMod[2, 7]'], desc: t('A square root of a modulo p.', 'Une racine carrée de a modulo p.', 'a の p を法とする平方根。') },
      { name: 'DiscreteLog', syntax: 'DiscreteLog[b, t, m]', examples: ['DiscreteLog[3, 4, 7]'], desc: t('Least x with b^x ≡ t (mod m).', 'Plus petit x tel que b^x ≡ t (mod m).', 'b^x ≡ t (mod m) を満たす最小の x。') },
      { name: 'EvenQ', syntax: 'EvenQ[n]', examples: ['EvenQ[4]'], desc: t('True if n is even.', 'Vrai si n est pair.', 'n が偶数なら True。') },
      { name: 'OddQ', syntax: 'OddQ[n]', examples: ['OddQ[7]'], desc: t('True if n is odd.', 'Vrai si n est impair.', 'n が奇数なら True。') },
      { name: 'IntegerQ', syntax: 'IntegerQ[x]', examples: ['IntegerQ[4]', 'IntegerQ[1/2]'], desc: t('True if x is an integer.', 'Vrai si x est un entier.', 'x が整数なら True。') },
    ],
  },
  {
    id: 'rationals',
    title: t('Rationals', 'Rationnels', '有理数'),
    fns: [
      { name: 'Numerator', syntax: 'Numerator[x]', examples: ['Numerator[6/10]'], desc: t('Numerator in lowest terms.', 'Numérateur (forme réduite).', '既約分数の分子。') },
      { name: 'Denominator', syntax: 'Denominator[x]', examples: ['Denominator[6/10]'], desc: t('Denominator in lowest terms.', 'Dénominateur (forme réduite).', '既約分数の分母。') },
      { name: 'ContinuedFraction', syntax: 'ContinuedFraction[x]', examples: ['ContinuedFraction[7/3]'], desc: t('Continued-fraction terms of a rational.', "Termes de la fraction continue d'un rationnel.", '有理数の連分数展開の各項。') },
      { name: 'FromContinuedFraction', syntax: 'FromContinuedFraction[{…}]', examples: ['FromContinuedFraction[{2, 3}]'], desc: t('Rebuild a rational from its terms.', 'Reconstruit un rationnel à partir de ses termes.', '各項から有理数を復元。') },
      { name: 'Rationalize', syntax: 'Rationalize[x, d]', examples: ['Rationalize[314/100, 10]'], desc: t('Best rational with denominator ≤ d.', 'Meilleur rationnel de dénominateur ≤ d.', '分母が d 以下の最良の有理数近似。') },
    ],
  },
  {
    id: 'elementary',
    title: t('Elementary & special functions', 'Fonctions élémentaires et spéciales', '初等・特殊関数'),
    fns: [
      { name: 'Exp', syntax: 'Exp[x]', examples: ['Exp[1]', 'Exp[I*Pi]'], desc: t('Exponential eˣ (accepts complex).', 'Exponentielle eˣ (accepte les complexes).', '指数関数 eˣ（複素数対応）。') },
      { name: 'Log', syntax: 'Log[x]  ·  Log[b, x]', examples: ['Log[E]', 'Log[2, 8]', 'Log[-1]'], desc: t('Natural log, or base-b log; complex for negative x.', 'Logarithme naturel, ou en base b ; complexe pour x négatif.', '自然対数、または底 b の対数。x が負なら複素数。') },
      { name: 'Log2', syntax: 'Log2[x]', examples: ['Log2[8]'], desc: t('Base-2 logarithm.', 'Logarithme en base 2.', '底 2 の対数。') },
      { name: 'Log10', syntax: 'Log10[x]', examples: ['Log10[1000]'], desc: t('Base-10 logarithm.', 'Logarithme décimal (base 10).', '常用対数（底 10）。') },
      { name: 'Sin', syntax: 'Sin[x]', examples: ['Sin[Pi/4]', 'N[Sin[1], 20]'], desc: t('Sine (radians; accepts complex).', 'Sinus (radians ; accepte les complexes).', '正弦（ラジアン、複素数対応）。') },
      { name: 'Cos', syntax: 'Cos[x]', examples: ['Cos[Pi/3]'], desc: t('Cosine (radians; accepts complex).', 'Cosinus (radians ; accepte les complexes).', '余弦（ラジアン、複素数対応）。') },
      { name: 'Tan', syntax: 'Tan[x]', examples: ['Tan[Pi/4]'], desc: t('Tangent (radians).', 'Tangente (radians).', '正接（ラジアン）。') },
      { name: 'ArcSin', syntax: 'ArcSin[x]', examples: ['ArcSin[1]'], desc: t('Inverse sine.', 'Arc sinus.', '逆正弦。') },
      { name: 'ArcCos', syntax: 'ArcCos[x]', examples: ['ArcCos[0]'], desc: t('Inverse cosine.', 'Arc cosinus.', '逆余弦。') },
      { name: 'ArcTan', syntax: 'ArcTan[x]  ·  ArcTan[x, y]', examples: ['ArcTan[1]', 'ArcTan[1, 1]'], desc: t('Inverse tangent; the two-argument form is atan2(y, x).', 'Arc tangente ; la forme à deux arguments est atan2(y, x).', '逆正接。2 引数形は atan2(y, x)。') },
      { name: 'Sinh', syntax: 'Sinh[x]', examples: ['Sinh[1]'], desc: t('Hyperbolic sine.', 'Sinus hyperbolique.', '双曲線正弦。') },
      { name: 'Cosh', syntax: 'Cosh[x]', examples: ['Cosh[0]'], desc: t('Hyperbolic cosine.', 'Cosinus hyperbolique.', '双曲線余弦。') },
      { name: 'Tanh', syntax: 'Tanh[x]', examples: ['Tanh[1]'], desc: t('Hyperbolic tangent.', 'Tangente hyperbolique.', '双曲線正接。') },
      { name: 'ArcSinh', syntax: 'ArcSinh[x]', examples: ['ArcSinh[1]'], desc: t('Inverse hyperbolic sine.', 'Argument sinus hyperbolique.', '逆双曲線正弦。') },
      { name: 'ArcCosh', syntax: 'ArcCosh[x]', examples: ['ArcCosh[2]'], desc: t('Inverse hyperbolic cosine.', 'Argument cosinus hyperbolique.', '逆双曲線余弦。') },
      { name: 'ArcTanh', syntax: 'ArcTanh[x]', examples: ['ArcTanh[1/2]'], desc: t('Inverse hyperbolic tangent.', 'Argument tangente hyperbolique.', '逆双曲線正接。') },
      { name: 'Erf', syntax: 'Erf[x]', examples: ['Erf[1]'], desc: t('The error function.', "La fonction d'erreur.", '誤差関数。') },
      { name: 'Erfc', syntax: 'Erfc[x]', examples: ['Erfc[1]'], desc: t('The complementary error function, 1 − Erf[x].', "La fonction d'erreur complémentaire, 1 − Erf[x].", '相補誤差関数（1 − Erf[x]）。') },
      { name: 'Zeta', syntax: 'Zeta[s]', examples: ['Zeta[2]', 'Zeta[4]'], desc: t('The Riemann zeta function.', 'La fonction zêta de Riemann.', 'リーマンゼータ関数。') },
      { name: 'Gamma', syntax: 'Gamma[x]', examples: ['Gamma[5]', 'N[Gamma[1/2], 20]'], desc: t('The gamma function (Gamma[n] = (n−1)!).', 'La fonction gamma (Gamma[n] = (n−1)!).', 'ガンマ関数（Gamma[n] = (n−1)!）。') },
      { name: 'LogGamma', syntax: 'LogGamma[x]', examples: ['LogGamma[10]'], desc: t('The logarithm of the gamma function.', 'Le logarithme de la fonction gamma.', 'ガンマ関数の対数。') },
      { name: 'BesselJ', syntax: 'BesselJ[n, x]', examples: ['N[BesselJ[0, 1], 15]'], desc: t('The Bessel function of the first kind, order n.', 'La fonction de Bessel de première espèce, ordre n.', '第一種ベッセル関数（次数 n）。') },
      { name: 'BesselI', syntax: 'BesselI[n, x]', examples: ['N[BesselI[1, 2], 15]'], desc: t('The modified Bessel function of the first kind, order n.', 'La fonction de Bessel modifiée de première espèce, ordre n.', '第一種変形ベッセル関数（次数 n）。') },
      { name: 'BesselY', syntax: 'BesselY[n, x]', examples: ['N[BesselY[0, 1], 15]'], desc: t('The Bessel function of the second kind, order n.', 'La fonction de Bessel de seconde espèce, ordre n.', '第二種ベッセル関数（次数 n）。') },
      { name: 'BesselK', syntax: 'BesselK[n, x]', examples: ['N[BesselK[1, 1], 15]'], desc: t('The modified Bessel function of the second kind, order n.', 'La fonction de Bessel modifiée de seconde espèce, ordre n.', '第二種変形ベッセル関数（次数 n）。') },
      { name: 'Beta', syntax: 'Beta[a, b]', examples: ['Beta[2, 3]', 'N[Beta[1/2, 1/2], 20]'], desc: t('The Euler beta function B(a, b) = Γ(a)Γ(b)/Γ(a+b).', 'La fonction bêta d’Euler B(a, b) = Γ(a)Γ(b)/Γ(a+b).', 'オイラーのベータ関数 B(a, b) = Γ(a)Γ(b)/Γ(a+b)。') },
      { name: 'PolyGamma', syntax: 'PolyGamma[n, x]', examples: ['N[PolyGamma[1], 15]', 'N[PolyGamma[1, 1], 15]'], desc: t('PolyGamma[x] is the digamma ψ(x); PolyGamma[n, x] is the nth polygamma.', 'PolyGamma[x] est la digamma ψ(x) ; PolyGamma[n, x] est la nᵉ polygamma.', 'PolyGamma[x] はディガンマ ψ(x)、PolyGamma[n, x] は第 n ポリガンマ。') },
      { name: 'Identify', syntax: 'Identify[x]', examples: ['Identify[Pi^2/6]', 'Identify[Zeta[3]]', 'Identify[Sqrt[2] + 1]'], desc: t('Inverse symbolic calculator — guess a closed form for a high-precision number (π²/6, √2 + 1, …).', 'Calculatrice symbolique inverse — devine une forme close pour un nombre en haute précision (π²/6, √2 + 1, …).', '逆記号計算機 — 高精度の数から閉じた形を推定（π²/6、√2 + 1 など）。') },
    ],
  },
  {
    id: 'complex',
    title: t('Complex', 'Nombres complexes', '複素数'),
    fns: [
      { name: 'Re', syntax: 'Re[z]', examples: ['Re[3 + 4*I]'], desc: t('Real part.', 'Partie réelle.', '実部。') },
      { name: 'Im', syntax: 'Im[z]', examples: ['Im[3 + 4*I]'], desc: t('Imaginary part.', 'Partie imaginaire.', '虚部。') },
      { name: 'Conjugate', syntax: 'Conjugate[z]', examples: ['Conjugate[3 + 4*I]'], desc: t('Complex conjugate.', 'Conjugué complexe.', '複素共役。') },
      { name: 'Arg', syntax: 'Arg[z]', examples: ['Arg[I]', 'Arg[-1]'], desc: t('Argument (phase) in radians.', 'Argument (phase) en radians.', '偏角（ラジアン）。') },
    ],
  },
  {
    id: 'linearalgebra',
    title: t('Linear algebra', 'Algèbre linéaire', '線形代数'),
    fns: [
      { name: 'Det', syntax: 'Det[m]', examples: ['Det[{{1, 2}, {3, 4}}]', 'Det[D[{x + y, x y}, {{x, y}}]]'], desc: t('Determinant of a square matrix — entries may be numbers or polynomials (e.g. a Jacobian from D).', "Déterminant d'une matrice carrée — à coefficients numériques ou polynomiaux (p. ex. une jacobienne issue de D).", '正方行列の行列式。成分は数または多項式（D のヤコビ行列など）。') },
      { name: 'Inverse', syntax: 'Inverse[m]', examples: ['Inverse[{{1, 2}, {3, 4}}]'], desc: t('Matrix inverse.', "Inverse d'une matrice.", '逆行列。') },
      { name: 'Transpose', syntax: 'Transpose[m]', examples: ['Transpose[{{1, 2}, {3, 4}}]'], desc: t('Matrix transpose.', "Transposée d'une matrice.", '転置行列。') },
      { name: 'Dot', syntax: 'Dot[a, b]', examples: ['Dot[{{1, 2}, {3, 4}}, {{0, 1}, {1, 0}}]'], desc: t('Matrix product.', 'Produit matriciel.', '行列の積。') },
      { name: 'MatrixRank', syntax: 'MatrixRank[m]', examples: ['MatrixRank[{{1, 2}, {2, 4}}]'], desc: t('Rank of a matrix.', "Rang d'une matrice.", '行列の階数（ランク）。') },
      { name: 'Eigenvalues', syntax: 'Eigenvalues[m]', examples: ['Eigenvalues[{{2, 1}, {1, 2}}]', 'Eigenvalues[{{0, 1}, {2, 0}}]'], desc: t('Exact real eigenvalues of a rational square matrix (as radicals where possible); complex eigenvalues are omitted.', 'Valeurs propres réelles exactes (en radicaux si possible) ; les complexes sont omises.', '有理正方行列の厳密な実固有値（可能なら根号で）。複素固有値は省略。') },
      { name: 'LinearSolve', syntax: 'LinearSolve[m, b]', examples: ['LinearSolve[{{1, 1}, {1, -1}}, {3, 1}]'], desc: t('Solve the linear system m·x = b.', 'Résout le système linéaire m·x = b.', '連立一次方程式 m·x = b を解く。') },
      { name: 'IdentityMatrix', syntax: 'IdentityMatrix[n]', examples: ['IdentityMatrix[3]'], desc: t('The n×n identity matrix.', 'La matrice identité n×n.', 'n×n の単位行列。') },
      { name: 'LatticeReduce', syntax: 'LatticeReduce[{{…}, …}]', examples: ['LatticeReduce[{{1, 1, 1}, {-1, 0, 2}, {3, 5, 6}}]'], desc: t('LLL-reduced basis of an integer lattice.', "Base réduite (LLL) d'un réseau entier.", '整数格子の LLL 簡約基底。') },
    ],
  },
  {
    id: 'calculus',
    title: t('Calculus', 'Analyse', '解析'),
    fns: [
      { name: 'D', syntax: 'D[f, x]  ·  D[f, {x, n}]  ·  D[f, {{x, y, …}}]', examples: ['D[x^3 + x, x]', 'D[x^2 y, {{x, y}}]', 'Det[D[{x + y, x y}, {{x, y}}]]'], desc: t('Symbolic derivative of a polynomial expression: ∂f/∂x, the n-th derivative, the gradient, or (of a list of functions) the Jacobian matrix. Free symbols are the variables; juxtaposition multiplies (2 x, x y).', "Dérivée symbolique d'une expression polynomiale : ∂f/∂x, la dérivée n-ième, le gradient, ou (d'une liste de fonctions) la matrice jacobienne. Les symboles libres sont les variables ; la juxtaposition multiplie (2 x, x y).", '多項式式の記号微分：∂f/∂x、n 階微分、勾配、または（関数のリストなら）ヤコビ行列。自由記号が変数となり、並置は乗算（2 x, x y）。') },
    ],
  },
  {
    id: 'plotting',
    title: t('Plotting', 'Tracés', 'グラフ描画'),
    fns: [
      { name: 'Plot', syntax: 'Plot[expr, {x, a, b}]', examples: ['Plot[Sin[x], {x, 0, 2*Pi}]', 'Plot[{Sin[x], Cos[x]}, {x, -Pi, Pi}]'], desc: t('Plot a function of one variable over a range (a list plots several curves). Hover to read values.', "Trace une fonction d'une variable sur un intervalle (une liste trace plusieurs courbes). Survolez pour lire les valeurs.", '1 変数関数を区間上に描画（リストで複数曲線）。ホバーで値を表示。') },
      { name: 'Plot3D', syntax: 'Plot3D[expr, {x, a, b}, {y, c, d}]', examples: ['Plot3D[Sin[x]*Cos[y], {x, -3, 3}, {y, -3, 3}]'], desc: t('Surface plot of a function of two variables — drag to rotate, scroll to zoom, hover for (x, y, z).', 'Tracé de surface pour une fonction de deux variables — glisser pour tourner, molette pour zoomer, survol pour (x, y, z).', '2 変数関数の曲面を描画。ドラッグで回転、スクロールでズーム、ホバーで (x, y, z) を表示。') },
    ],
  },
  {
    id: 'elliptic',
    title: t('Elliptic curves', 'Courbes elliptiques', '楕円曲線'),
    fns: [
      { name: 'ECAdd', syntax: 'ECAdd[a, b, p, P, Q]', examples: ['ECAdd[2, 2, 17, {5, 1}, {6, 3}]'], desc: t('Group sum P + Q on y² = x³ + a·x + b over GF(p). Points are {x, y}; the identity (point at infinity) is {}.', 'Somme P + Q sur y² = x³ + a·x + b dans GF(p). Les points sont {x, y} ; l’identité (point à l’infini) est {}.', 'GF(p) 上の y² = x³ + a·x + b における群和 P + Q。点は {x, y}、無限遠点（単位元）は {}。') },
      { name: 'ECMultiply', syntax: 'ECMultiply[a, b, p, k, P]', examples: ['ECMultiply[2, 2, 17, 5, {5, 1}]'], desc: t('Scalar multiple k·P (repeated addition) over GF(p).', 'Multiple scalaire k·P (addition répétée) dans GF(p).', 'GF(p) 上のスカラー倍 k·P（反復加算）。') },
      { name: 'ECOrder', syntax: 'ECOrder[a, b, p]', examples: ['ECOrder[2, 2, 17]'], desc: t('The number of points on the curve over GF(p), including the point at infinity.', 'Le nombre de points de la courbe sur GF(p), point à l’infini compris.', 'GF(p) 上の曲線の点の数（無限遠点を含む）。') },
      { name: 'ECPointOrder', syntax: 'ECPointOrder[a, b, p, P]', examples: ['ECPointOrder[2, 2, 17, {5, 1}]'], desc: t('The order of a point P — the least k > 0 with k·P = O.', 'L’ordre d’un point P — le plus petit k > 0 tel que k·P = O.', '点 P の位数 — k·P = O となる最小の k > 0。') },
      { name: 'ECPointQ', syntax: 'ECPointQ[a, b, p, P]', examples: ['ECPointQ[2, 2, 17, {5, 1}]'], desc: t('True if the point lies on the curve over GF(p).', 'Vrai si le point est sur la courbe dans GF(p).', '点が GF(p) 上の曲線上にあれば True。') },
      { name: 'ECDiscriminant', syntax: 'ECDiscriminant[a, b]', examples: ['ECDiscriminant[2, 2]'], desc: t('The discriminant Δ = −16(4a³ + 27b²); zero iff the curve is singular.', 'Le discriminant Δ = −16(4a³ + 27b²) ; nul ssi la courbe est singulière.', '判別式 Δ = −16(4a³ + 27b²)。曲線が特異な場合に限り 0。') },
      { name: 'ECjInvariant', syntax: 'ECjInvariant[a, b]', examples: ['ECjInvariant[2, 2]'], desc: t('The j-invariant j = 1728·4a³ / (4a³ + 27b²).', 'Le j-invariant j = 1728·4a³ / (4a³ + 27b²).', 'j 不変量 j = 1728·4a³ / (4a³ + 27b²)。') },
    ],
  },
  {
    id: 'random',
    title: t('Randomness', 'Aléatoire', '乱数'),
    fns: [
      { name: 'RandomInteger', syntax: 'RandomInteger[{min, max}]', examples: ['RandomInteger[100]', 'RandomInteger[{-5, 5}]', 'RandomInteger[6, 5]'], desc: t('A uniform random integer in [0, n] or [min, max]; a second argument gives a list. Cryptographically secure.', 'Un entier aléatoire uniforme dans [0, n] ou [min, max] ; un second argument renvoie une liste. Cryptographiquement sûr.', '[0, n] または [min, max] の一様乱整数。第 2 引数でリスト。暗号学的に安全。') },
      { name: 'RandomReal', syntax: 'RandomReal[{min, max}]', examples: ['RandomReal[]', 'RandomReal[{0, 1}]', 'RandomReal[1, 3]'], desc: t('A uniform random real in [0, 1), [0, max) or [min, max), at full precision; a second argument gives a list.', 'Un réel aléatoire uniforme dans [0, 1), [0, max) ou [min, max), en pleine précision ; un second argument renvoie une liste.', '[0, 1)・[0, max)・[min, max) の一様乱実数（フル精度）。第 2 引数でリスト。') },
      { name: 'RandomChoice', syntax: 'RandomChoice[list]', examples: ['RandomChoice[{2, 3, 5, 7}]', 'RandomChoice[{1, 2, 3}, 4]'], desc: t('Pick a uniform random element of a list (with replacement); a second argument gives that many picks.', "Choisit un élément aléatoire uniforme d'une liste (avec remise) ; un second argument en renvoie plusieurs.", 'リストから一様ランダムに要素を選ぶ（復元）。第 2 引数でその個数。') },
      { name: 'RandomPrime', syntax: 'RandomPrime[max]', examples: ['RandomPrime[1000]', 'RandomPrime[{100, 200}]'], desc: t('A random prime in [2, max] or [min, max].', 'Un nombre premier aléatoire dans [2, max] ou [min, max].', '[2, max] または [min, max] の乱素数。') },
      { name: 'RandomBytes', syntax: 'RandomBytes[n]', examples: ['RandomBytes[16]', 'RandomBytes[32]'], desc: t('n cryptographically-secure random bytes as a hex string (keys, nonces, salts).', 'n octets aléatoires cryptographiquement sûrs sous forme hexadécimale (clés, nonces, sels).', '暗号学的に安全な n バイトを 16 進文字列で（鍵・ノンス・ソルト）。') },
    ],
  },
  {
    id: 'solving',
    title: t('Solving & logic', 'Résolution et logique', '求解・論理'),
    fns: [
      { name: 'SatisfiableQ', syntax: 'SatisfiableQ[constraint]', examples: ['SatisfiableQ[x > 5 && x < 8]', 'SatisfiableQ[p || q]'], desc: t('True if the constraints are satisfiable (linear arithmetic / propositional logic).', 'Vrai si les contraintes sont satisfiables (arithmétique linéaire / logique propositionnelle).', '制約が充足可能なら True（線形算術・命題論理）。') },
      { name: 'TautologyQ', syntax: 'TautologyQ[formula]', examples: ['TautologyQ[Implies[p && Implies[p, q], q]]', 'TautologyQ[x + 1 > x, Reals]'], desc: t('True if the formula is valid — true for every assignment.', 'Vrai si la formule est valide — vraie pour toute affectation.', '式が恒真（すべての割り当てで真）なら True。') },
      { name: 'FindInstance', syntax: 'FindInstance[c, vars]  ·  [c, vars, dom]', examples: ['FindInstance[x + y == 10 && x - y == 2, {x, y}]', 'FindInstance[2*x == 3, {x}, Reals]'], desc: t('One assignment satisfying the constraints; domain Integers (default) or Reals.', 'Une affectation satisfaisant les contraintes ; domaine Integers (défaut) ou Reals.', '制約を満たす一つの割り当て。領域は Integers（既定）または Reals。') },
      { name: 'Solve', syntax: 'Solve[c, vars]', examples: ['Solve[x^2 == 2, x]', 'Solve[x^2 - x - 1 == 0, x]', 'Solve[x + y == 4 && x >= 0 && y >= 0, {x, y}]'], desc: t('A univariate polynomial equation gives exact real roots (radicals where possible, e.g. ±√2); other constraints give all integer solutions (or one real). Exact typeset rules.', 'Une équation polynomiale à une variable donne des racines réelles exactes (radicaux si possible, ex. ±√2) ; sinon toutes les solutions entières (ou une réelle). Règles exactes.', '1 変数多項式方程式は厳密な実根（可能なら根号、例 ±√2）。それ以外は整数全解（または実数 1 つ）。') },
      { name: 'Maximize', syntax: 'Maximize[obj, constraints, {vars}]', examples: ['Maximize[x + y, x + 2*y <= 14 && 3*x - y >= 0 && x - y <= 2, {x, y}, Reals]'], desc: t('Maximize a linear objective subject to constraints → {optimum, {x -> …}}.', 'Maximise un objectif linéaire sous contraintes → {optimum, {x -> …}}.', '制約下で線形目的関数を最大化 → {最適値, {x -> …}}。') },
      { name: 'Minimize', syntax: 'Minimize[obj, constraints, {vars}]', examples: ['Minimize[y, y >= x && x >= 3, {x, y}]'], desc: t('Minimize a linear objective subject to constraints.', 'Minimise un objectif linéaire sous contraintes.', '制約下で線形目的関数を最小化。') },
      { name: 'SMT', syntax: 'SMT["…"]', examples: ['SMT["(declare-const x Int)(assert (> x 5))(assert (< x 7))(check-sat)(get-value (x))"]'], desc: t('Run a raw SMT-LIB 2 script through the z3rs solver.', 'Exécute un script SMT-LIB 2 brut via le solveur z3rs.', 'SMT-LIB 2 スクリプトを z3rs ソルバーで実行。') },
    ],
  },
]
