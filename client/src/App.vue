<script setup lang="ts">
import { onMounted, provide, ref } from 'vue'
import { checkServer, getConfig } from './api'
import FileBrowser from './views/FileBrowser.vue'
import SettingsView from './views/SettingsView.vue'
import { themeMode, setThemeMode, type ThemeMode } from './theme'
import { checkForUpdates } from './updater'

const view = ref<'browser' | 'settings'>('browser')
const connected = ref<boolean | null>(null)
const hasUpdate = ref(false)
const fileBrowserRef = ref<InstanceType<typeof FileBrowser> | null>(null)

function goHome() {
  view.value = 'browser'
  // 等待视图切换后，重置选中的文件夹
  setTimeout(() => {
    if (fileBrowserRef.value) {
      fileBrowserRef.value.goHome()
    }
  }, 0)
}

const toasts = ref<{ id: number; msg: string; type: string }[]>([])
let toastId = 0

function toast(msg: string, type = 'error') {
  const id = ++toastId
  toasts.value.push({ id, msg, type })
  setTimeout(() => {
    toasts.value = toasts.value.filter((t) => t.id !== id)
  }, 4000)
}
provide('toast', toast)

const themeOptions: { value: ThemeMode; label: string; title: string }[] = [
  { value: 'system', label: '跟随系统', title: '跟随系统' },
  { value: 'light', label: '白天', title: '白天' },
  { value: 'dark', label: '晚上', title: '晚上' },
  { value: 'green', label: '护眼', title: '护眼模式' },
]

async function refreshConn() {
  const cfg = getConfig()
  if (!cfg.url) {
    connected.value = false
    return
  }
  connected.value = await checkServer(cfg.url, cfg.token)
}

function onSaved() {
  refreshConn()
}

onMounted(async () => {
  await refreshConn()
  
  try {
    const update = await checkForUpdates()
    if (update) {
      hasUpdate.value = true
    }
  } catch (error) {
    console.error('启动时检查更新失败:', error)
  }
})
</script>

<template>
  <div class="app">
    <header class="topbar">
      <div class="brand" title="返回主页" @click="goHome">
        <svg
          class="logo"
          viewBox="0 0 20 20"
          width="18"
          height="18"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <path d="M3 17 L17 17 L3 3 Z" />
          <path d="M3 17 L10 7" />
          <path d="M7 17 L12 10" />
          <path d="M11.5 17 L15 12" />
        </svg>
        <span class="title">PlotVault PDM</span>
      </div>
      <div class="conn" :class="connected === true ? 'ok' : connected === false ? 'bad' : 'unknown'">
        <span class="dot"></span>
        <span>{{ connected === true ? '已连接' : connected === false ? '未连接' : '…' }}</span>
        <button v-if="connected === false" class="conn-go" @click="view = 'settings'">去设置</button>
      </div>
      <div class="theme-toggle" role="radiogroup" aria-label="主题">
        <button
          v-for="o in themeOptions"
          :key="o.value"
          class="theme-opt"
          :class="{ active: themeMode === o.value }"
          :title="o.title"
          role="radio"
          :aria-checked="themeMode === o.value"
          @click="setThemeMode(o.value)"
        >
          <svg
            v-if="o.value === 'system'"
            class="opt-ico"
            viewBox="0 0 16 16"
            width="14"
            height="14"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <rect x="1.5" y="2.5" width="13" height="9" rx="1" />
            <path d="M8 11.5V14M5.5 14h5" />
          </svg>
          <svg
            v-else-if="o.value === 'light'"
            class="opt-ico"
            viewBox="0 0 16 16"
            width="14"
            height="14"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <circle cx="8" cy="8" r="3" />
            <path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M12.6 3.4l-1.4 1.4M4.8 11.2l-1.4 1.4" />
          </svg>
          <svg
            v-else-if="o.value === 'dark'"
            class="opt-ico"
            viewBox="0 0 16 16"
            width="14"
            height="14"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path d="M13.5 9.2A5.5 5.5 0 1 1 6.8 2.5a4.5 4.5 0 0 0 6.7 6.7z" />
          </svg>
          <svg
            v-else
            class="opt-ico"
            viewBox="0 0 16 16"
            width="14"
            height="14"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path d="M8 1.5v13M4 4l4-2.5 4 2.5M4 12l4 2.5 4-2.5" />
            <circle cx="8" cy="7" r="2" />
          </svg>
          <span class="opt-label">{{ o.label }}</span>
        </button>
      </div>
      <div class="top-ops">
        <button class="nav-btn" :class="{ active: view === 'browser' }" @click="view = 'browser'">文件</button>
        <button class="nav-btn update-available" :class="{ active: view === 'settings' }" @click="view = 'settings'">
          设置
          <span v-if="hasUpdate" class="update-dot" />
        </button>
      </div>
    </header>
    <main class="app-main">
      <FileBrowser v-if="view === 'browser'" ref="fileBrowserRef" />
      <SettingsView v-else @saved="onSaved" />
    </main>

    <div class="toast-wrap" aria-live="polite">
      <div v-for="t in toasts" :key="t.id" class="toast" :class="t.type">{{ t.msg }}</div>
    </div>
  </div>
</template>

<style scoped>
.app {
  height: 100%;
  display: flex;
  flex-direction: column;
}
.topbar {
  display: flex;
  align-items: center;
  gap: 16px;
  height: var(--topbar-h);
  padding: 0 16px;
  background: var(--bg-panel);
  border-bottom: 1px solid var(--border);
  flex-shrink: 0;
}
.brand {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  border-radius: var(--radius-sm);
  padding: 4px 6px;
  margin-left: -6px;
}
.brand:hover {
  background: var(--bg-hover);
}
.logo {
  color: var(--accent);
  flex-shrink: 0;
}
.title {
  font-weight: 700;
  font-size: var(--font-lg);
}
.conn {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: var(--font-sm);
  color: var(--text-dim);
  margin-left: 4px;
}
.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--text-faint);
}
.conn.ok .dot {
  background: var(--ok);
}
.conn.bad .dot {
  background: var(--danger);
}
.conn.bad {
  color: var(--danger);
}
.conn-go {
  font-size: var(--font-sm);
  padding: 1px 8px;
  border: 1px solid var(--accent);
  color: var(--accent);
  background: transparent;
  border-radius: 10px;
  line-height: 1.5;
}
.conn-go:hover:not(:disabled) {
  background: var(--accent-soft);
  color: var(--accent);
}
.theme-toggle {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 2px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg);
}
.theme-opt {
  display: flex;
  align-items: center;
  gap: 4px;
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
  color: var(--text-dim);
  padding: 3px 8px;
  font-size: var(--font-sm);
}
.theme-opt:hover:not(:disabled) {
  background: var(--bg-hover);
  color: var(--text);
}
.theme-opt.active {
  background: var(--accent-soft);
  color: var(--accent);
}
.opt-ico {
  flex-shrink: 0;
}
.top-ops {
  display: flex;
  gap: 6px;
}
.nav-btn {
  background: transparent;
  border: none;
  color: var(--text-dim);
  padding: 4px 10px;
  border-radius: var(--radius-sm);
  font-size: var(--font-base);
}
.nav-btn:hover:not(:disabled) {
  background: var(--bg-hover);
  color: var(--text);
}
.nav-btn.active {
  background: var(--bg-active);
  color: var(--text);
  font-weight: 600;
}
.update-available {
  position: relative;
}
.update-dot {
  position: absolute;
  top: 2px;
  right: 2px;
  width: 6px;
  height: 6px;
  background: #ff4d4f;
  border-radius: 50%;
  animation: pulse 2s infinite;
}
@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}
.app-main {
  flex: 1;
  min-height: 0;
}
@media (max-width: 1000px) {
  .theme-opt .opt-label {
    display: none;
  }
}
</style>