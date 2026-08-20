import { createApp } from 'vue'
import App from './App.vue'
import { initTheme } from './theme'
import './styles.css'

initTheme() // 幂等：与防闪脚本一致 + 注册系统主题监听
createApp(App).mount('#app')