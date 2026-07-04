<script setup lang="ts">
import { computed } from 'vue'
import katex from 'katex'

const props = defineProps<{ tex: string }>()

const html = computed(() =>
  katex.renderToString(props.tex, {
    displayMode: true,
    throwOnError: false,
    output: 'html',
    strict: false,
  }),
)
</script>

<template>
  <!-- KaTeX output is generated from our own engine, not user HTML. -->
  <div class="math" v-html="html"></div>
</template>

<style scoped>
.math {
  overflow-x: auto;
  overflow-y: hidden;
  padding: 0.1rem 0;
}
</style>
