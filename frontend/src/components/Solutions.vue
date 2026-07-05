<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import katex from 'katex'
import type { Solutions } from '../engine'

const props = defineProps<{ data: Solutions }>()
const { t } = useI18n({ useScope: 'global' })

function kx(tex: string): string {
  return katex.renderToString(tex, { throwOnError: false, output: 'html' })
}

const headers = computed(() => props.data.vars.map(kx))
const cells = computed(() => props.data.rows.map((r) => r.map(kx)))
const countLabel = computed(() => t('sol.count', { n: props.data.count }, props.data.count))
</script>

<template>
  <div class="solutions">
    <div class="sol-caption">
      {{ countLabel }}
      <span v-if="data.truncated" class="sol-trunc">· {{ t('sol.truncated', { n: data.count }) }}</span>
    </div>
    <div v-if="data.count > 0" class="sol-scroll">
      <table class="sol-table">
        <thead>
          <tr>
            <th v-for="(h, i) in headers" :key="i" v-html="h"></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(row, ri) in cells" :key="ri">
            <td v-for="(c, ci) in row" :key="ci" v-html="c"></td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>
