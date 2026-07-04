<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { Graphics } from '../engine'

const props = defineProps<{ data: Graphics }>()

const COLORS = ['#e7b85c', '#4fd6e0', '#e19a90', '#8fb8c9']

function fmt(v: number): string {
  if (v === 0) return '0'
  const a = Math.abs(v)
  if (a >= 1e4 || a < 1e-3) return v.toExponential(2)
  return Number(v.toPrecision(4)).toString()
}

/* ------------------------------------------------------------------ 2D ---- */
const W = 640
const H = 360
const padL = 52
const padR = 18
const padT = 16
const padB = 30

const svg2d = ref<SVGSVGElement | null>(null)
const plot2d = computed(() => (props.data.kind === 'plot2d' ? props.data : null))

const bounds2d = computed(() => {
  const p = plot2d.value
  if (!p) return null
  let ymin = Infinity
  let ymax = -Infinity
  for (const s of p.series)
    for (const [, y] of s.points)
      if (y != null && Number.isFinite(y)) {
        if (y < ymin) ymin = y
        if (y > ymax) ymax = y
      }
  if (!Number.isFinite(ymin)) return null
  if (ymin === ymax) {
    ymin -= 1
    ymax += 1
  }
  const pad = (ymax - ymin) * 0.06
  return { xmin: p.xmin, xmax: p.xmax, ymin: ymin - pad, ymax: ymax + pad }
})

function sx(x: number) {
  const b = bounds2d.value!
  return padL + ((x - b.xmin) / (b.xmax - b.xmin)) * (W - padL - padR)
}
function sy(y: number) {
  const b = bounds2d.value!
  return padT + ((b.ymax - y) / (b.ymax - b.ymin)) * (H - padT - padB)
}

const paths = computed(() => {
  const p = plot2d.value
  if (!p || !bounds2d.value) return []
  return p.series.map((s, i) => {
    let d = ''
    let pen = false
    for (const [x, y] of s.points) {
      if (y == null || !Number.isFinite(y)) {
        pen = false
        continue
      }
      d += `${pen ? 'L' : 'M'}${sx(x).toFixed(2)} ${sy(y).toFixed(2)} `
      pen = true
    }
    return { d, color: COLORS[i % COLORS.length] }
  })
})

function ticks(lo: number, hi: number): number[] {
  const out: number[] = []
  for (let k = 0; k <= 4; k++) out.push(lo + ((hi - lo) * k) / 4)
  return out
}
const xticks = computed(() => (bounds2d.value ? ticks(bounds2d.value.xmin, bounds2d.value.xmax) : []))
const yticks = computed(() => (bounds2d.value ? ticks(bounds2d.value.ymin, bounds2d.value.ymax) : []))
const zeroX = computed(() => (bounds2d.value && 0 >= bounds2d.value.xmin && 0 <= bounds2d.value.xmax ? sx(0) : null))
const zeroY = computed(() => (bounds2d.value && 0 >= bounds2d.value.ymin && 0 <= bounds2d.value.ymax ? sy(0) : null))

// hover
interface Hover2d {
  vx: number
  dataX: number
  items: { color: string; y: number; vy: number }[]
  px: number
  py: number
  flip: boolean
}
const hover2d = ref<Hover2d | null>(null)

function onMove2d(e: PointerEvent) {
  const p = plot2d.value
  const svg = svg2d.value
  if (!p || !bounds2d.value || !svg) return
  const rect = svg.getBoundingClientRect()
  const scale = rect.width / W
  const svgX = (e.clientX - rect.left) / scale
  const N = p.series[0]?.points.length ?? 0
  if (!N) return
  const frac = (svgX - padL) / (W - padL - padR)
  let idx = Math.round(frac * (N - 1))
  idx = Math.max(0, Math.min(N - 1, idx))
  const dataX = p.series[0].points[idx][0]
  const items = p.series
    .map((s, i) => {
      const y = s.points[idx][1]
      return y != null && Number.isFinite(y) ? { color: COLORS[i % COLORS.length], y, vy: sy(y) } : null
    })
    .filter((v): v is { color: string; y: number; vy: number } => v != null)
  if (!items.length) {
    hover2d.value = null
    return
  }
  const vx = sx(dataX)
  hover2d.value = { vx, dataX, items, px: vx * scale, py: 8, flip: vx * scale > rect.width * 0.62 }
}
function onLeave2d() {
  hover2d.value = null
}
const tip2dStyle = computed(() => {
  const h = hover2d.value
  if (!h) return {}
  return {
    left: `${h.px}px`,
    top: `${h.py}px`,
    transform: h.flip ? 'translateX(calc(-100% - 12px))' : 'translateX(12px)',
  }
})

/* ------------------------------------------------------------------ 3D ---- */
const canvas = ref<HTMLCanvasElement | null>(null)
let yaw = -0.6
let pitch = 0.5
let zoom = 1
let dragging = false
let lastX = 0
let lastY = 0
let ro: ResizeObserver | null = null
let verts3d: { sx: number; sy: number; x: number; y: number; z: number }[] = []
const hover3d = ref<{ x: number; y: number; z: number; px: number; py: number } | null>(null)

function color3d(t: number): string {
  const lo = [78, 120, 128]
  const hi = [231, 184, 92]
  const c = lo.map((l, i) => Math.round(l + (hi[i] - l) * t))
  return `rgb(${c[0]},${c[1]},${c[2]})`
}

function draw3d() {
  const p = props.data
  const cv = canvas.value
  if (p.kind !== 'plot3d' || !cv) return
  const ctx = cv.getContext('2d')
  if (!ctx) return

  const dpr = window.devicePixelRatio || 1
  const cssW = cv.clientWidth || 600
  const cssH = 380
  cv.width = Math.round(cssW * dpr)
  cv.height = Math.round(cssH * dpr)
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  ctx.clearRect(0, 0, cssW, cssH)

  const ny = p.z.length
  const nx = p.z[0]?.length ?? 0
  if (nx < 2 || ny < 2) return

  let zmin = Infinity
  let zmax = -Infinity
  for (const row of p.z)
    for (const v of row)
      if (v != null && Number.isFinite(v)) {
        if (v < zmin) zmin = v
        if (v > zmax) zmax = v
      }
  if (!Number.isFinite(zmin)) return
  if (zmin === zmax) zmax = zmin + 1

  const ca = Math.cos(yaw)
  const sa = Math.sin(yaw)
  const cp = Math.cos(pitch)
  const sp = Math.sin(pitch)
  const cx = cssW / 2
  const cy = cssH / 2 + 30
  const scale = Math.min(cssW, cssH) * 0.34 * zoom
  const norm = (v: number, min: number, max: number) => ((v - min) / (max - min)) * 2 - 1

  const dataX = (i: number) => p.xmin + (i / (nx - 1)) * (p.xmax - p.xmin)
  const dataY = (j: number) => p.ymin + (j / (ny - 1)) * (p.ymax - p.ymin)

  type P = { X: number; Y: number; depth: number; t: number; x: number; y: number; z: number }
  const grid: (P | null)[][] = []
  verts3d = []
  for (let j = 0; j < ny; j++) {
    const row: (P | null)[] = []
    for (let i = 0; i < nx; i++) {
      const zv = p.z[j][i]
      if (zv == null || !Number.isFinite(zv)) {
        row.push(null)
        continue
      }
      const x = norm(i, 0, nx - 1)
      const y = norm(j, 0, ny - 1)
      const zz = norm(zv, zmin, zmax)
      const x1 = x * ca - y * sa
      const y1 = x * sa + y * ca
      const up = y1 * sp + zz * cp
      const depth = y1 * cp - zz * sp
      const pt: P = { X: cx + scale * x1, Y: cy - scale * up, depth, t: (zz + 1) / 2, x: dataX(i), y: dataY(j), z: zv }
      row.push(pt)
      verts3d.push({ sx: pt.X, sy: pt.Y, x: pt.x, y: pt.y, z: zv })
    }
    grid.push(row)
  }

  type Quad = { pts: [number, number][]; depth: number; t: number }
  const quads: Quad[] = []
  for (let j = 0; j < ny - 1; j++) {
    for (let i = 0; i < nx - 1; i++) {
      const c = [grid[j][i], grid[j][i + 1], grid[j + 1][i + 1], grid[j + 1][i]]
      if (c.some((v) => v == null)) continue
      const cs = c as P[]
      quads.push({
        pts: cs.map((v) => [v.X, v.Y] as [number, number]),
        depth: cs.reduce((s, v) => s + v.depth, 0) / 4,
        t: cs.reduce((s, v) => s + v.t, 0) / 4,
      })
    }
  }
  quads.sort((a, b) => b.depth - a.depth)

  ctx.lineJoin = 'round'
  for (const q of quads) {
    ctx.beginPath()
    ctx.moveTo(q.pts[0][0], q.pts[0][1])
    for (let k = 1; k < 4; k++) ctx.lineTo(q.pts[k][0], q.pts[k][1])
    ctx.closePath()
    ctx.fillStyle = color3d(q.t)
    ctx.fill()
    ctx.strokeStyle = 'rgba(14,25,21,0.35)'
    ctx.lineWidth = 0.5
    ctx.stroke()
  }
}

function onDown3d(e: PointerEvent) {
  dragging = true
  lastX = e.clientX
  lastY = e.clientY
  hover3d.value = null
  ;(e.target as HTMLElement).setPointerCapture(e.pointerId)
}
function onMove3d(e: PointerEvent) {
  if (dragging) {
    yaw += (e.clientX - lastX) * 0.01
    pitch = Math.max(0.08, Math.min(1.5, pitch + (e.clientY - lastY) * 0.008))
    lastX = e.clientX
    lastY = e.clientY
    draw3d()
    return
  }
  const cv = canvas.value
  if (!cv) return
  const rect = cv.getBoundingClientRect()
  const mx = e.clientX - rect.left
  const my = e.clientY - rect.top
  let best: (typeof verts3d)[number] | null = null
  let bd = 16 * 16
  for (const v of verts3d) {
    const d = (v.sx - mx) ** 2 + (v.sy - my) ** 2
    if (d < bd) {
      bd = d
      best = v
    }
  }
  hover3d.value = best ? { x: best.x, y: best.y, z: best.z, px: best.sx, py: best.sy } : null
}
function onUp3d() {
  dragging = false
}
function onLeave3d() {
  hover3d.value = null
}
function onWheel3d(e: WheelEvent) {
  e.preventDefault()
  zoom = Math.max(0.4, Math.min(4, zoom * (e.deltaY < 0 ? 1.1 : 0.9)))
  draw3d()
}
const tip3dStyle = computed(() => {
  const h = hover3d.value
  if (!h) return {}
  return { left: `${h.px}px`, top: `${h.py}px` }
})

onMounted(() => {
  if (props.data.kind === 'plot3d') {
    draw3d()
    ro = new ResizeObserver(() => draw3d())
    if (canvas.value) ro.observe(canvas.value)
  }
})
onBeforeUnmount(() => ro?.disconnect())
watch(() => props.data, draw3d)
</script>

<template>
  <div class="graphics">
    <!-- 2D -->
    <div v-if="plot2d && bounds2d" class="plot2d-wrap">
      <svg
        ref="svg2d"
        class="plot2d"
        :viewBox="`0 0 ${W} ${H}`"
        preserveAspectRatio="xMidYMid meet"
        role="img"
        @pointermove="onMove2d"
        @pointerleave="onLeave2d"
      >
        <rect :x="padL" :y="padT" :width="W - padL - padR" :height="H - padT - padB" class="plot-bg" />
        <line v-for="(tx, i) in xticks" :key="'gx' + i" :x1="sx(tx)" :x2="sx(tx)" :y1="padT" :y2="H - padB" class="grid" />
        <line v-for="(ty, i) in yticks" :key="'gy' + i" :x1="padL" :x2="W - padR" :y1="sy(ty)" :y2="sy(ty)" class="grid" />
        <line v-if="zeroY != null" :x1="padL" :x2="W - padR" :y1="zeroY" :y2="zeroY" class="axis" />
        <line v-if="zeroX != null" :x1="zeroX" :x2="zeroX" :y1="padT" :y2="H - padB" class="axis" />
        <text v-for="(tx, i) in xticks" :key="'tx' + i" :x="sx(tx)" :y="H - padB + 16" class="tick tick-x">{{ fmt(tx) }}</text>
        <text v-for="(ty, i) in yticks" :key="'ty' + i" :x="padL - 6" :y="sy(ty) + 3" class="tick tick-y">{{ fmt(ty) }}</text>
        <path v-for="(pth, i) in paths" :key="'p' + i" :d="pth.d" :stroke="pth.color" class="curve" />
        <template v-if="hover2d">
          <line :x1="hover2d.vx" :x2="hover2d.vx" :y1="padT" :y2="H - padB" class="crosshair" />
          <circle v-for="(it, i) in hover2d.items" :key="'d' + i" :cx="hover2d.vx" :cy="it.vy" r="3.5" :fill="it.color" class="hover-dot" />
        </template>
      </svg>
      <div v-if="hover2d" class="plot-tip" :style="tip2dStyle">
        <div class="tip-x">x = {{ fmt(hover2d.dataX) }}</div>
        <div v-for="(it, i) in hover2d.items" :key="'r' + i" class="tip-row">
          <span class="tip-sw" :style="{ background: it.color }"></span>{{ fmt(it.y) }}
        </div>
      </div>
    </div>

    <!-- 3D -->
    <div v-else-if="data.kind === 'plot3d'" class="plot3d-wrap">
      <canvas
        ref="canvas"
        class="plot3d"
        @pointerdown="onDown3d"
        @pointermove="onMove3d"
        @pointerup="onUp3d"
        @pointercancel="onUp3d"
        @pointerleave="onLeave3d"
        @wheel="onWheel3d"
      ></canvas>
      <div v-if="hover3d" class="plot-tip tip-3d" :style="tip3dStyle">
        x = {{ fmt(hover3d.x) }}<br />y = {{ fmt(hover3d.y) }}<br />z = {{ fmt(hover3d.z) }}
      </div>
      <div class="plot-hint">drag · scroll</div>
    </div>
  </div>
</template>
