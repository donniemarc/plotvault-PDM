<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { DEFAULT_SERVER, getConfig, setConfig } from '../api'
import { themeMode, setThemeMode, type ThemeMode } from '../theme'
import { getVersion } from '@tauri-apps/api/app'
import { checkForUpdates, getDownloadUrl, type UpdateInfo } from '../updater'
import { open } from '@tauri-apps/plugin-shell'

const NAS_ROOT_KEY = 'plotvault_pdm_nas_root'

const emit = defineEmits<{ saved: [] }>()

const url = ref(getConfig().url || DEFAULT_SERVER)
const token = ref(getConfig().token)
const nasRoot = ref(localStorage.getItem(NAS_ROOT_KEY) || '')
const testing = ref(false)
const result = ref<{ ok: boolean; msg: string } | null>(null)
const appVersion = ref('')
const hasUpdate = ref(false)
const isCheckingUpdate = ref(false)
const updateInfo = ref<UpdateInfo | null>(null)
const updateResult = ref<{ ok: boolean; msg: string } | null>(null)

const themeOptions: { value: ThemeMode; label: string; desc: string }[] = [
  { value: 'system', label: '跟随系统', desc: '与操作系统外观保持一致' },
  { value: 'light', label: '白天', desc: '明亮清爽，适合白天使用' },
  { value: 'dark', label: '晚上', desc: '沉浸深色，适合夜间审图' },
  { value: 'green', label: '护眼', desc: '绿色调护眼，减少视觉疲劳' },
]

onMounted(async () => {
  try {
    appVersion.value = await getVersion()
  } catch {
    appVersion.value = 'unknown'
  }
  
  try {
    const update = await checkForUpdates()
    if (update) {
      hasUpdate.value = true
      updateInfo.value = update
    }
  } catch (error) {
    console.error('检查更新失败:', error)
  }
})

async function test() {
  testing.value = true
  result.value = null
  const base = url.value.trim().replace(/\/+$/, '')
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), 8000)
  try {
    const resp = await fetch(base + '/api/health', {
      headers: token.value.trim() ? { Authorization: `Bearer ${token.value.trim()}` } : undefined,
      signal: controller.signal,
    })
    if (resp.ok) {
      result.value = { ok: true, msg: '连接成功' }
    } else if (resp.status === 401 || resp.status === 403) {
      result.value = { ok: false, msg: '连接失败：Token 与服务器不一致，请检查 API Token' }
    } else {
      result.value = { ok: false, msg: `连接失败：服务器返回 HTTP ${resp.status}` }
    }
  } catch (e: any) {
    if (e?.name === 'AbortError') {
      result.value = { ok: false, msg: '连接超时：请确认后端容器已启动，并检查地址与端口' }
    } else {
      result.value = { ok: false, msg: '无法连接服务器：请确认后端容器已启动，地址请使用 NAS 的 IP（如 http://192.168.1.100:8642）' }
    }
  } finally {
    clearTimeout(timer)
    testing.value = false
  }
}

function save() {
  setConfig({ url: url.value, token: token.value })
  localStorage.setItem(NAS_ROOT_KEY, nasRoot.value.trim())
  result.value = { ok: true, msg: '已保存' }
  emit('saved')
}

async function checkUpdate() {
  isCheckingUpdate.value = true
  updateResult.value = null
  try {
    const update = await checkForUpdates()
    if (update) {
      hasUpdate.value = true
      updateInfo.value = update
    } else {
      hasUpdate.value = false
      updateInfo.value = null
      updateResult.value = { ok: true, msg: '已是最新版本' }
    }
  } catch (error) {
    updateResult.value = { ok: false, msg: '检查更新失败' }
  } finally {
    isCheckingUpdate.value = false
  }
}

async function goToDownload() {
  if (updateInfo.value) {
    const url = getDownloadUrl(updateInfo.value)
    await open(url)
  }
}

function formatChangelog(text: string) {
  return text.replace(/\n/g, '<br>')
}


</script>

<template>
  <div class="settings">
    <div class="card">
      <h3>连接设置</h3>
      <label class="field">
        <span>服务器地址</span>
        <input
          v-model="url"
          type="text"
          :placeholder="DEFAULT_SERVER"
        />
        <div class="dim small">NAS 上运行的后端地址（Docker 部署后使用 NAS 的 IP 和端口）</div>
      </label>
      <label class="field">
        <span>API Token（可选）</span>
        <input v-model="token" type="password" placeholder="未设置则不填" />
        <div class="dim small">与 docker-compose 中 API_TOKEN 保持一致</div>
      </label>
      <label class="field">
        <span>NAS 文件映射路径（可选）</span>
        <input v-model="nasRoot" type="text" placeholder="如 Z:\" />
        <div class="dim small">SMB 映射到本地的盘符路径，用于右键打开文件所在文件夹</div>
      </label>

      <div class="actions">
        <button :disabled="testing" @click="test">{{ testing ? '测试中…' : '测试连接' }}</button>
        <button class="primary" @click="save">保存</button>
      </div>

      <div v-if="result" class="result" :class="result.ok ? 'ok' : 'err'">{{ result.msg }}</div>
    </div>

    <div class="card">
      <h3>外观</h3>
      <div class="theme-options" role="radiogroup" aria-label="主题">
        <label
          v-for="o in themeOptions"
          :key="o.value"
          class="theme-option"
          :class="{ active: themeMode === o.value }"
        >
          <input
            type="radio"
            name="settings-theme"
            :value="o.value"
            :checked="themeMode === o.value"
            @change="setThemeMode(o.value)"
          />
          <svg
            v-if="o.value === 'system'"
            class="opt-ico"
            viewBox="0 0 16 16"
            width="16"
            height="16"
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
            width="16"
            height="16"
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
            width="16"
            height="16"
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
            width="16"
            height="16"
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
          <span class="theme-option-text">
            <b>{{ o.label }}</b>
            <span class="dim small">{{ o.desc }}</span>
          </span>
        </label>
      </div>
      <div class="dim small theme-note">切换即时生效，无需重启；重启后恢复上次选择。</div>
    </div>

    <div class="card">
      <h3>关于</h3>
      <div class="about-info">
        <div class="about-row">
          <span class="about-label">应用名称</span>
          <span class="about-value">PlotVault PDM</span>
        </div>
        <div class="about-row">
          <span class="about-label">当前版本</span>
          <span class="about-value">v{{ appVersion }}</span>
        </div>
        <div class="about-row">
          <span class="about-label">产品描述</span>
          <span class="about-value">轻量级个人 NAS 图纸文档管理系统</span>
        </div>
        <div class="about-row">
          <span class="about-label">检查更新</span>
          <div class="update-wrapper">
            <button @click="checkUpdate" :disabled="isCheckingUpdate" class="check-update-btn">
              {{ isCheckingUpdate ? '检查中...' : '检查更新' }}
            </button>
            <span v-if="hasUpdate" class="update-dot" />
            <span v-if="updateResult" class="update-result" :class="updateResult.ok ? 'ok' : 'err'">
              {{ updateResult.msg }}
            </span>
          </div>
        </div>
      </div>
      
      <div v-if="updateInfo" class="update-info">
        <p class="update-title">发现新版本: <strong>{{ updateInfo.version }}</strong></p>
        <div class="changelog" v-html="formatChangelog(updateInfo.changelog)" />
        <button @click="goToDownload" class="download-btn">
          前往下载
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings {
  height: 100%;
  overflow: auto;
  padding: 32px;
  display: flex;
  flex-direction: row;
  flex-wrap: wrap;
  gap: 16px;
  align-items: flex-start;
}
.card {
  flex: 1 1 480px;
  max-width: 600px;
  min-width: 380px;
  background: var(--bg-panel);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  box-shadow: var(--shadow-sm);
  padding: 20px;
}
.card h3 {
  margin: 0 0 16px;
  font-size: var(--font-lg);
}
.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 16px;
}
.field span {
  color: var(--text-dim);
  font-size: var(--font-sm);
}
.field input {
  width: 100%;
}
.small {
  font-size: var(--font-sm);
}
.actions {
  display: flex;
  gap: 8px;
}
.result {
  margin-top: 14px;
}
.result.ok {
  color: var(--ok);
}
.result.err {
  color: var(--danger);
}
.theme-options {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.theme-option {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background var(--transition-fast), border-color var(--transition-fast);
}
.theme-option:hover {
  background: var(--bg-hover);
}
.theme-option.active {
  background: var(--accent-soft);
  border-color: var(--accent);
}
.theme-option input[type='radio'] {
  width: 16px;
  height: 16px;
  margin: 0;
  accent-color: var(--accent);
  flex-shrink: 0;
}
.opt-ico {
  color: var(--text-dim);
  flex-shrink: 0;
}
.theme-option.active .opt-ico {
  color: var(--accent);
}
.theme-option-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.theme-option-text b {
  font-size: var(--font-base);
}
.theme-note {
  margin-top: 10px;
  color: var(--text-faint);
}
.about-info {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.about-row {
  display: flex;
  align-items: center;
  gap: 12px;
}
.about-label {
  color: var(--text-dim);
  font-size: var(--font-sm);
  min-width: 80px;
}
.about-value {
  font-weight: 500;
}
.update-wrapper {
  display: flex;
  align-items: center;
  gap: 8px;
}
.check-update-btn {
  padding: 6px 12px;
  background: var(--bg-panel);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  cursor: pointer;
  font-size: var(--font-sm);
  transition: background var(--transition-fast);
}
.check-update-btn:hover:not(:disabled) {
  background: var(--bg-hover);
}
.check-update-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
.update-dot {
  width: 8px;
  height: 8px;
  background: #ff4d4f;
  border-radius: 50%;
  animation: pulse 2s infinite;
}
.update-result {
  font-size: var(--font-sm);
  margin-left: 4px;
}
.update-result.ok {
  color: var(--ok);
}
.update-result.err {
  color: var(--danger);
}
@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}
.update-info {
  margin-top: 16px;
  padding: 12px;
  background: var(--bg-secondary);
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
}
.update-title {
  margin: 0 0 8px;
  color: var(--accent);
}
.changelog {
  font-size: var(--font-sm);
  color: var(--text-dim);
  line-height: 1.6;
  max-height: 120px;
  overflow-y: auto;
  margin-bottom: 12px;
}
.download-btn {
  padding: 8px 16px;
  background: var(--accent);
  color: var(--text-on-accent);
  border: none;
  border-radius: var(--radius-sm);
  cursor: pointer;
  font-weight: 500;
  transition: opacity var(--transition-fast);
}
.download-btn:hover {
  opacity: 0.9;
}
</style>