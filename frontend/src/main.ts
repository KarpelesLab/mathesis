import { createApp } from 'vue'

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

createApp(App).mount('#app')
