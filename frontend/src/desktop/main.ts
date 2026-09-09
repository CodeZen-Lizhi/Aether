import { createApp } from 'vue'
import { createI18n } from '@/i18n'
import '@/style.css'
import './desktop.css'
import DesktopApp from './DesktopApp.vue'

createApp(DesktopApp).use(createI18n()).mount('#app')
