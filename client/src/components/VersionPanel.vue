<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { api } from '../api'
import type { FileMeta, VersionInfo } from '../types'
import { formatBytes, formatDate, saveBlob } from '../utils'
import UploadDialog from './UploadDialog.vue'

const props = defineProps<{ file: FileMeta }>()
const emit = defineEmits<{ close: [] }>()

const versions = ref<VersionInfo[]>([])
const loading = ref(true)
const err = ref('')
const uploadOpen = ref(false)

async function load() {
  loading.value = true
  err.value = ''
  try {
    const r = await api.versions(props.file.id)
    versions.value = r.versions
  } catch (e: any) {
    err.value = e?.message || String(e)
  } finally {
    loading.value = false
  }
}

async function download(v: VersionInfo) {
  try {
    const name = v.version_no === props.file.current_version ? props.file.name : `${props.file.name.replace(/\.[^.]+$/, '')}_v${v.version_no}.${props.file.ext}`
    const blob = await api.fetchBlob(api.downloadUrl(props.file.id, v.version_no))
    await saveBlob(name, blob)
  } catch (e: any) {
    err.value = e?.message || String(e)
  }
}

onMounted(load)
</script>

<template>
  <div class="panel">
    <div class="panel-head">
      <b>版本历史</b>
      <div class="head-right">
        <button class="primary" @click="uploadOpen = true">上传新版本</button>
        <button @click="emit('close')">关闭</button>
      </div>
    </div>
    <div class="panel-body">
      <div v-if="loading" class="dim">加载中…</div>
      <div v-else-if="err" class="err">{{ err }}</div>
      <div v-else-if="versions.length === 0" class="dim">暂无版本</div>
      <div v-else class="v-list">
        <div
          v-for="v in [...versions].reverse()"
          :key="v.id"
          class="v-item"
          :class="{ current: v.version_no === file.current_version }"
        >
          <div class="v-line">
            <span class="v-no">v{{ v.version_no }}</span>
            <span v-if="v.version_no === file.current_version" class="tag">当前</span>
            <span class="dim">{{ formatDate(v.created_at) }}</span>
            <span class="dim">{{ formatBytes(v.size) }}</span>
            <span class="spacer"></span>
            <button @click="download(v)">下载</button>
          </div>
          <div v-if="v.comment" class="v-comment">
            <svg
              class="v-comment-ico"
              viewBox="0 0 16 16"
              width="12"
              height="12"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <path d="M8 2.5a5.5 5.5 0 0 0-4.77 8.23L2.5 13.5l2.9-.75A5.5 5.5 0 1 0 8 2.5z" />
            </svg>
            <span>{{ v.comment }}</span>
          </div>
          <div class="v-sha dim">{{ v.sha256.slice(0, 16) }}…</div>
        </div>
      </div>
    </div>
    <UploadDialog
      v-if="uploadOpen"
      mode="version"
      folder-name=""
      :folder-id="null"
      :target-file="file"
      @done="uploadOpen = false; load()"
      @close="uploadOpen = false"
    />
  </div>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  height: 100%;
}
.panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 12px;
  border-bottom: 1px solid var(--border);
}
.head-right {
  display: flex;
  gap: 6px;
}
.panel-body {
  flex: 1;
  overflow: auto;
  padding: 10px 12px;
}
.v-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.v-item {
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 8px 10px;
}
.v-item.current {
  border-color: var(--accent);
  background: var(--accent-soft);
}
.v-line {
  display: flex;
  align-items: center;
  gap: 8px;
}
.v-no {
  font-weight: 600;
}
.tag {
  background: var(--accent-strong);
  color: var(--text-on-accent);
  font-size: var(--font-xs);
  padding: 0 6px;
  border-radius: 4px;
}
.spacer {
  flex: 1;
}
.v-comment {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-top: 4px;
  color: var(--text);
}
.v-comment-ico {
  color: var(--text-dim);
  flex-shrink: 0;
}
.v-sha {
  margin-top: 2px;
  font-size: 11px;
}
.err {
  color: var(--danger);
}
.dim {
  color: var(--text-dim);
}
</style>
