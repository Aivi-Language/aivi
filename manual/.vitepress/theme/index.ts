import DefaultTheme from 'vitepress/theme'
import Layout from './Layout.vue'
import '@fontsource/jetbrains-mono/400.css'
import '@fontsource/jetbrains-mono/500.css'
import '@fontsource/jetbrains-mono/700.css'
import './table-cards.css'
import './fira-code.css'

export default {
  extends: DefaultTheme,
  Layout,
}
