<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { LANGS, saveLang, type Lang } from '../i18n'

const { locale, t } = useI18n({ useScope: 'global' })

function change(e: Event) {
  const v = (e.target as HTMLSelectElement).value as Lang
  locale.value = v
  saveLang(v)
  document.documentElement.lang = v
}
</script>

<template>
  <label class="lang">
    <span class="sr-only">{{ t('nav.lang') }}</span>
    <select :value="locale" :aria-label="t('nav.lang')" @change="change">
      <option v-for="l in LANGS" :key="l.code" :value="l.code">{{ l.label }}</option>
    </select>
  </label>
</template>
