<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { Graphics } from '../engine'

const props = defineProps<{ data: Graphics }>()

const COLORS = ['#e7b85c', '#4fd6e0', '#e19a90', '#8fb8c9']

/* ------------------------------------------------------------------ 2D ---- */
const W = 640
const H = 360
const padL = 52
const padR = 18
const padT = 16
const padB = 30

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
function fmt(v: number): string {
  if (v === 0) return '0'
  const a = Math.abs(v)
  if (a >= 1e4 || a < 1e-3) return v.toExponential(1)
  return Number(v.toPrecision(3)).toString()
}

const xticks = computed(() => (bounds2d.value ? ticks(bounds2d.value.xmin, bounds2d.value.xmax) : []))
const yticks = computed(() => (bounds2d.value ? ticks(bounds2d.value.ymin, bounds2d.value.ymax) : []))
const zeroX = computed(() => (bounds2d.value && 0 >= bounds2d.value.xmin && 0 <= bounds2d.value.xmax ? sx(0) : null))
const zeroY = computed(() => (bounds2d.value && 0 >= bounds2d.value.ymin && 0 <= bounds2d.value.ymax ? sy(0) : null))

/* ------------------------------------------------------------------ 3D ---- */
const canvas = ref<HTMLCanvasElement | null>(null)
let yaw = -0.6
let pitch = 0.5
let dragging = false
let lastX = 0
let lastY = 0
let ro: ResizeObserver | null = null

function color3d(t: number): string {
  // teal (low) → amber (high)
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
  const scale = Math.min(cssW, cssH) * 0.34

  const norm = (v: number, min: number, max: number) => ((v - min) / (max - min)) * 2 - 1
  const project = (i: number, j: number, z: number) => {
    const x = norm(i, 0, nx - 1)
    const y = norm(j, 0, ny - 1)
    const zz = norm(z, zmin, zmax)
    const x1 = x * ca - y * sa
    const y1 = x * sa + y * ca
    const up = y1 * sp + zz * cp
    const depth = y1 * cp - zz * sp
    return { X: cx + scale * x1, Y: cy - scale * up, depth, t: (zz + 1) / 2 }
  }

  type Quad = { pts: [number, number][]; depth: number; t: number }
  const quads: Quad[] = []
  for (let j = 0; j < ny - 1; j++) {
    for (let i = 0; i < nx - 1; i++) {
      const zs = [p.z[j][i], p.z[j][i + 1], p.z[j + 1][i + 1], p.z[j + 1][i]]
      if (zs.some((v) => v == null || !Number.isFinite(v))) continue
      const idx: [number, number][] = [
        [i, j],
        [i + 1, j],
        [i + 1, j + 1],
        [i, j + 1],
      ]
      let depth = 0
      let t = 0
      const pts = idx.map(([ii, jj], k) => {
        const pr = project(ii, jj, zs[k] as number)
        depth += pr.depth
        t += pr.t
        return [pr.X, pr.Y] as [number, number]
      })
      quads.push({ pts, depth: depth / 4, t: t / 4 })
    }
  }
  // Painter's algorithm: farthest first.
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

function onDown(e: PointerEvent) {
  dragging = true
  lastX = e.clientX
  lastY = e.clientY
  ;(e.target as HTMLElement).setPointerCapture(e.pointerId)
}
function onMove(e: PointerEvent) {
  if (!dragging) return
  yaw += (e.clientX - lastX) * 0.01
  pitch = Math.max(0.08, Math.min(1.5, pitch + (e.clientY - lastY) * 0.008))
  lastX = e.clientX
  lastY = e.clientY
  draw3d()
}
function onUp() {
  dragging = false
}

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
    <svg
      v-if="plot2d && bounds2d"
      class="plot2d"
      :viewBox="`0 0 ${W} ${H}`"
      preserveAspectRatio="xMidYMid meet"
      role="img"
    >
      <rect :x="padL" :y="padT" :width="W - padL - padR" :height="H - padT - padB" class="plot-bg" />
      <line
        v-for="(tx, i) in xticks"
        :key="'gx' + i"
        :x1="sx(tx)"
        :x2="sx(tx)"
        :y1="padT"
        :y2="H - padB"
        class="grid"
      />
      <line
        v-for="(ty, i) in yticks"
        :key="'gy' + i"
        :x1="padL"
        :x2="W - padR"
        :y1="sy(ty)"
        :y2="sy(ty)"
        class="grid"
      />
      <line v-if="zeroY != null" :x1="padL" :x2="W - padR" :y1="zeroY" :y2="zeroY" class="axis" />
      <line v-if="zeroX != null" :x1="zeroX" :x2="zeroX" :y1="padT" :y2="H - padB" class="axis" />
      <text v-for="(tx, i) in xticks" :key="'tx' + i" :x="sx(tx)" :y="H - padB + 16" class="tick tick-x">{{ fmt(tx) }}</text>
      <text v-for="(ty, i) in yticks" :key="'ty' + i" :x="padL - 6" :y="sy(ty) + 3" class="tick tick-y">{{ fmt(ty) }}</text>
      <path v-for="(pth, i) in paths" :key="'p' + i" :d="pth.d" :stroke="pth.color" class="curve" />
    </svg>

    <!-- 3D -->
    <canvas
      v-else-if="data.kind === 'plot3d'"
      ref="canvas"
      class="plot3d"
      @pointerdown="onDown"
      @pointermove="onMove"
      @pointerup="onUp"
      @pointercancel="onUp"
    ></canvas>
  </div>
</template>
