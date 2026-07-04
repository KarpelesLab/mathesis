import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// Deployed under https://karpeleslab.github.io/mathesis/, so assets must be
// requested from the `/mathesis/` base. The wasm engine is loaded via
// `new URL('…_bg.wasm', import.meta.url)`, which Vite rewrites to a hashed,
// base-prefixed asset automatically.
export default defineConfig({
  base: '/mathesis/',
  plugins: [vue()],
})
