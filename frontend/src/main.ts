import { createApp, watchEffect } from 'vue'

// Self-hosted type — bundled and served from Pages, so the notebook stays
// instant and works offline. Spectral (scholarly serif) + IBM Plex Mono
// (computation) frame KaTeX's own mathematical faces.
import '@fontsource/spectral/500.css'
import '@fontsource/spectral/600.css'
import '@fontsource/spectral/700.css'
import '@fontsource/spectral/500-italic.css'
import '@fontsource/ibm-plex-mono/400.css'
import '@fontsource/ibm-plex-mono/500.css'

import 'katex/dist/katex.min.css'
import './style.css'
import App from './App.vue'
import { i18n } from './i18n'

// Keep <html lang> and the tab title in step with the chosen UI language (the
// static index.html carries the English title/description for crawlers).
watchEffect(() => {
  const locale = i18n.global.locale.value
  document.documentElement.lang = locale
  document.title = `Mathesis — ${i18n.global.t('tagline')}`
})

createApp(App).use(i18n).mount('#app')
