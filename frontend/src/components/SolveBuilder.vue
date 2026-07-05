<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

const emit = defineEmits<{ (e: 'insert', expr: string): void; (e: 'close'): void }>()
const { t } = useI18n({ useScope: 'global' })

const constraints = ref<string[]>([''])
const solveFor = ref('')
const domain = ref<'Integers' | 'Reals'>('Integers')

function addConstraint() {
  constraints.value.push('')
}
function removeConstraint(i: number) {
  constraints.value.splice(i, 1)
  if (constraints.value.length === 0) constraints.value.push('')
}

const expr = computed(() => {
  const cs = constraints.value.map((c) => c.trim()).filter(Boolean)
  if (cs.length === 0) return ''
  const cons = cs.length > 1 ? cs.join(' && ') : cs[0]
  const vars = solveFor.value.split(',').map((v) => v.trim()).filter(Boolean)
  if (vars.length === 0) return ''
  const varsStr = vars.length === 1 ? vars[0] : `{${vars.join(', ')}}`
  const dom = domain.value === 'Reals' ? ', Reals' : ''
  return `Solve[${cons}, ${varsStr}${dom}]`
})

function insert() {
  if (expr.value) emit('insert', expr.value)
}
</script>

<template>
  <div class="builder-overlay" @click.self="emit('close')">
    <div class="builder" role="dialog" aria-modal="true" :aria-label="t('builder.title')">
      <header class="builder-head">
        <h2><span class="tf">∴</span> {{ t('builder.title') }}</h2>
        <button class="docs-close" :aria-label="t('builder.cancel')" @click="emit('close')">✕</button>
      </header>

      <div class="builder-body">
        <label class="builder-label">{{ t('builder.constraints') }}</label>
        <div v-for="(c, i) in constraints" :key="i" class="builder-row">
          <input
            v-model="constraints[i]"
            class="builder-input mono"
            :placeholder="t('builder.constraintPh')"
            @keydown.enter.prevent="addConstraint"
          />
          <button v-if="constraints.length > 1" class="builder-x" @click="removeConstraint(i)">✕</button>
        </div>
        <button class="builder-add" @click="addConstraint">+ {{ t('builder.add') }}</button>

        <div class="builder-grid">
          <div>
            <label class="builder-label">{{ t('builder.solveFor') }}</label>
            <input v-model="solveFor" class="builder-input mono" :placeholder="t('builder.solveForPh')" />
          </div>
          <div>
            <label class="builder-label">{{ t('builder.domain') }}</label>
            <select v-model="domain" class="builder-input">
              <option value="Integers">Integers</option>
              <option value="Reals">Reals</option>
            </select>
          </div>
        </div>

        <label class="builder-label">{{ t('builder.preview') }}</label>
        <pre class="builder-preview">{{ expr || 'Solve[…, …]' }}</pre>
      </div>

      <footer class="builder-foot">
        <button class="builder-cancel" @click="emit('close')">{{ t('builder.cancel') }}</button>
        <button class="builder-insert" :disabled="!expr" @click="insert">{{ t('builder.insert') }}</button>
      </footer>
    </div>
  </div>
</template>
