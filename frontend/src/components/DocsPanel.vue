<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { CATEGORIES } from '../docs'
import type { Lang } from '../i18n'

const emit = defineEmits<{ (e: 'close'): void; (e: 'insert', ex: string): void }>()
const { t, tm, locale } = useI18n({ useScope: 'global' })

const query = ref('')
const lang = computed(() => locale.value as Lang)

const filtered = computed(() => {
  const q = query.value.trim().toLowerCase()
  if (!q) return CATEGORIES
  return CATEGORIES.map((c) => ({
    ...c,
    fns: c.fns.filter(
      (f) =>
        f.name.toLowerCase().includes(q) ||
        f.syntax.toLowerCase().includes(q) ||
        f.desc[lang.value].toLowerCase().includes(q),
    ),
  })).filter((c) => c.fns.length > 0)
})

const guide = computed(() => tm('guide') as unknown as { h: string; p: string }[])
</script>

<template>
  <div class="docs-overlay" @click.self="emit('close')">
    <aside class="docs" role="dialog" aria-modal="true" :aria-label="t('docs.title')">
      <header class="docs-head">
        <h2>{{ t('docs.title') }} <span class="tf">∴</span></h2>
        <button class="docs-close" :aria-label="t('docs.close')" @click="emit('close')">✕</button>
      </header>

      <div class="docs-search">
        <input v-model="query" type="search" :placeholder="t('docs.search')" />
      </div>

      <div class="docs-body">
        <section v-if="!query" class="guide">
          <h3>{{ t('docs.guide') }}</h3>
          <div v-for="(g, i) in guide" :key="i" class="guide-item">
            <h4>{{ g.h }}</h4>
            <p>{{ g.p }}</p>
          </div>
        </section>

        <p v-if="filtered.length === 0" class="docs-empty">{{ t('docs.empty') }}</p>

        <section v-for="cat in filtered" :key="cat.id" class="docs-cat">
          <h3>{{ cat.title[lang] }}</h3>
          <div v-for="fn in cat.fns" :key="fn.name" class="fn">
            <code class="fn-syntax">{{ fn.syntax }}</code>
            <p class="fn-desc">{{ fn.desc[lang] }}</p>
            <div class="fn-ex">
              <button
                v-for="ex in fn.examples"
                :key="ex"
                class="fn-chip"
                :title="t('docs.insert')"
                @click="emit('insert', ex)"
              >
                {{ ex }}
              </button>
            </div>
          </div>
        </section>
      </div>
    </aside>
  </div>
</template>
