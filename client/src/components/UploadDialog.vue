<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { api } from '../api'
import type { FileMeta } from '../types'
import { formatBytes } from '../utils'

const props = defineProps<{
  mode: 'upload' | 'version'
  folderName: string
  folderId: number | null
  targetFile?: FileMeta | null
  initialFiles?: File[]
  existingNames?: string[]
}>()
const emit = defineEmits<{ done: []; close: [] }>()

const files = ref<File[]>([])
const comment = ref('')
const newFile = ref(false)
const uploading = ref(false)
const error = ref('')
const fileInput = ref<HTMLInputElement>()
const dupConfirmOpen = ref(false)
const doneCount = ref(0)

// 字节级进度：当前文件已传/总量 + 已完成文件字节累计 + 实时网速（滚动窗口）
const currentIdx = ref(0)
const curLoaded = ref(0)
const curTotal = ref(0)
const bytesDone = ref(0)
const bytesTotal = ref(0)
const speed = ref(0)
const etaSec = ref(0)
const speedSamples: { t: number; bytes: number }[] = []

function pick() {
  fileInput.value?.click()
}
function onPick(e: Event) {
  const input = e.target as HTMLInputElement
  if (input.files) files.value = Array.from(input.files)
}

onMounted(() => {
  if (props.initialFiles && props.initialFiles.length) {
    files.value = props.initialFiles
  }
})

const canSubmit = computed(() => files.value.length > 0 && !uploading.value)

// 与目标目录已存在文件重名的本地文件（默认行为：同名将作为新版本上传）
const dupes = computed(() =>
  files.value.filter((f) => (props.existingNames || []).includes(f.name.toLowerCase())),
)

const progressPct = computed(() => {
  if (!bytesTotal.value) return 0
  return Math.min(100, Math.round(((bytesDone.value + curLoaded.value) / bytesTotal.value) * 100))
})

const currentFile = computed(() => (currentIdx.value > 0 ? files.value[currentIdx.value - 1] : null))
const curFilePct = computed(() => {
  if (!curTotal.value) return 0
  return Math.min(100, Math.round((curLoaded.value / curTotal.value) * 100))
})
const transferredText = computed(() => formatBytes(bytesDone.value + curLoaded.value))
const speedText = computed(() => {
  const bps = speed.value
  if (!bps) return ''
  if (bps >= 1024 * 1024) return `${(bps / 1024 / 1024).toFixed(1)} MB/s`
  if (bps >= 1024) return `${(bps / 1024).toFixed(1)} KB/s`
  return `${Math.round(bps)} B/s`
})
const etaText = computed(() => {
  if (!speed.value || etaSec.value <= 0) return ''
  const s = etaSec.value
  if (s >= 3600) return `剩余约 ${(s / 3600).toFixed(1)} 小时`
  if (s >= 60) return `剩余约 ${Math.ceil(s / 60)} 分钟`
  return `剩余约 ${Math.ceil(s)} 秒`
})

function onFileProgress(loaded: number, total: number) {
  curLoaded.value = loaded
  curTotal.value = total || (currentFile.value?.size ?? 0)
  const bytes = bytesDone.value + loaded
  const now = performance.now()
  speedSamples.push({ t: now, bytes })
  while (speedSamples.length && now - speedSamples[0].t > 1500) speedSamples.shift()
  const first = speedSamples[0]
  if (first && now - first.t >= 300 && bytes > first.bytes) {
    speed.value = (bytes - first.bytes) / ((now - first.t) / 1000)
  }
  if (speed.value > 0) etaSec.value = Math.max(0, (bytesTotal.value - bytes) / speed.value)
}

function submit() {
  if (!canSubmit.value) return
  // 同名确认：默认作为「新版本」上传（勾选「同名作为新文件」则跳过确认，直接保留两者）
  if (props.mode === 'upload' && dupes.value.length && !newFile.value) {
    dupConfirmOpen.value = true
    return
  }
  void runUpload()
}

async function proceedAsNewVersion() {
  dupConfirmOpen.value = false
  await runUpload()
}

async function runUpload() {
  uploading.value = true
  error.value = ''
  doneCount.value = 0
  bytesDone.value = 0
  bytesTotal.value = files.value.reduce((s, f) => s + f.size, 0)
  currentIdx.value = 0
  curLoaded.value = 0
  curTotal.value = 0
  speed.value = 0
  etaSec.value = 0
  speedSamples.length = 0
  try {
    let ok = 0
    for (let i = 0; i < files.value.length; i++) {
      const f = files.value[i]
      currentIdx.value = i + 1
      curLoaded.value = 0
      curTotal.value = f.size
      if (props.mode === 'version' && props.targetFile) {
        await api.addVersionWithProgress(props.targetFile.id, f, comment.value, onFileProgress)
      } else {
        await api.uploadWithProgress(f, props.folderId, comment.value, newFile.value, onFileProgress)
      }
      ok++
      doneCount.value = ok
      bytesDone.value += f.size
      curLoaded.value = 0
      curTotal.value = 0
    }
    emit('done')
  } catch (e: any) {
    error.value = e?.message || String(e)
  } finally {
    uploading.value = false
  }
}
</script>

<template>
  <!-- I28：点击遮罩不关闭 -->
  <div class="modal-mask">
    <div class="modal">
      <h3>{{ mode === 'version' ? '上传新版本' : '上传文件' }}</h3>

      <div v-if="mode === 'version' && targetFile" class="info">
        目标文件：<b>{{ targetFile.name }}</b>（当前 v{{ targetFile.current_version }}）
      </div>
      <div v-else class="info">上传到目录：<b>{{ folderName || '根目录' }}</b></div>

      <input ref="fileInput" type="file" multiple style="display: none" @change="onPick" />
      <button @click="pick" :disabled="uploading">选择文件…</button>

      <div class="file-list">
        <div v-for="(f, i) in files" :key="i" class="file-item">
          <span>{{ f.name }}</span>
          <span class="dim">{{ (f.size / 1024 / 1024).toFixed(2) }} MB</span>
          <button class="remove" :disabled="uploading" @click="files.splice(i, 1)">×</button>
        </div>
      </div>

      <label class="field">
        <span>备注</span>
        <input v-model="comment" type="text" placeholder="版本说明（可选）" :disabled="uploading" />
      </label>

      <label v-if="mode === 'upload'" class="check">
        <input v-model="newFile" type="checkbox" :disabled="uploading" />
        同名文件作为新文件而不是新版本
      </label>

      <div v-if="mode === 'upload' && dupes.length" class="dup-warn">
        以下文件与目录中现有文件同名，默认将作为「新版本」上传：<br />
        <span v-for="d in dupes" :key="d.name" class="dup-item">{{ d.name }}</span>
      </div>

      <div class="note dim">
        默认行为：同一目录下重名文件将自动作为该文件的新版本。
      </div>

      <div v-if="uploading" class="progress-block">
        <div class="progress-track">
          <div class="progress-fill" :style="{ width: progressPct + '%' }"></div>
        </div>
        <div v-if="currentFile" class="progress">
          正在上传 {{ currentIdx }}/{{ files.length }}：<span class="prog-name">{{ currentFile.name }}</span>
          <span class="prog-pct">{{ curFilePct }}%</span>
        </div>
        <div class="progress dim">
          {{ transferredText }} / {{ formatBytes(bytesTotal) }}
          <template v-if="speedText">
            · {{ speedText }}<template v-if="etaText"> · {{ etaText }}</template>
          </template>
        </div>
      </div>
      <div v-if="error" class="error">{{ error }}</div>

      <div class="actions">
        <button :disabled="uploading" @click="emit('close')">取消</button>
        <button class="primary" :disabled="!canSubmit" @click="submit">
          {{ uploading ? '上传中…' : mode === 'version' ? '上传新版本' : '上传' }}
        </button>
      </div>
    </div>
  </div>

  <!-- 重名确认（应用内主题化模态框，替代原生 confirm） -->
  <div v-if="dupConfirmOpen" class="modal-mask">
    <div class="modal">
      <h3>将作为新版本上传</h3>
      <p class="dup-confirm-text">以下文件在目标目录中已存在同名文件：</p>
      <div class="dup-confirm-list">
        <span v-for="d in dupes" :key="d.name" class="dup-item">{{ d.name }}</span>
      </div>
      <p class="dup-confirm-note">
        继续将作为「新版本」上传。（勾选「同名文件作为新文件而不是新版本」可保留两者）
      </p>
      <div class="actions">
        <button @click="dupConfirmOpen = false">取消</button>
        <button class="primary" @click="proceedAsNewVersion">作为新版本上传</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.info {
  margin-bottom: 12px;
  color: var(--text-dim);
}
.file-list {
  margin: 12px 0;
  max-height: 180px;
  overflow: auto;
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 6px;
}
.file-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 6px;
}
.file-item span:first-child {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.remove {
  border: none;
  background: transparent;
  color: var(--danger);
  padding: 0 4px;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin: 10px 0;
}
.field span {
  color: var(--text-dim);
  font-size: var(--font-sm);
}
.check {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 8px 0;
  accent-color: var(--accent);
}
.note {
  font-size: var(--font-sm);
}
.dup-warn {
  margin: 8px 0;
  padding: 8px 10px;
  background: var(--warn-soft);
  border: 1px solid var(--warn-border);
  border-radius: var(--radius-sm);
  font-size: var(--font-sm);
  color: var(--warn);
}
.dup-item {
  display: inline-block;
  margin: 2px 8px 2px 0;
}
.progress-block {
  margin-top: 10px;
}
.progress-track {
  height: 4px;
  border-radius: 2px;
  background: var(--bg-hover);
  overflow: hidden;
}
.progress-fill {
  height: 100%;
  background: var(--accent);
  border-radius: 2px;
  transition: width 0.15s ease;
}
.progress {
  margin-top: 6px;
}
.prog-name {
  color: var(--text);
  word-break: break-all;
}
.prog-pct {
  color: var(--accent);
  font-weight: 600;
}
.error {
  margin-top: 10px;
  color: var(--danger);
}
.dup-confirm-text {
  margin: 0 0 8px;
}
.dup-confirm-list {
  margin-bottom: 8px;
  padding: 8px 10px;
  border: 1px solid var(--border-faint);
  border-radius: var(--radius-sm);
  background: var(--bg);
  max-height: 160px;
  overflow: auto;
}
.dup-confirm-note {
  margin: 0;
  font-size: var(--font-sm);
  color: var(--text-dim);
}
</style>