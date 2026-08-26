import { createApp } from 'vue'
import App from './App.vue'
import { initTheme } from './theme'
import './styles.css'

initTheme() // 幂等：与防闪脚本一致 + 注册系统主题监听

// 全局拦截右键，禁止 WebView 原生菜单（刷新/另存为/查看源代码等）
// 文件树/文件列表的自定义右键菜单通过 e.preventDefault() 自行处理，不受影响
document.addEventListener('contextmenu', (e) => e.preventDefault())

createApp(App).mount('#app')