<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue'
import { api, ApiError } from '../api'
import type { FileMeta, VersionInfo } from '../types'
import { isImage, isText, isArchive } from '../utils'
import DxfViewer from './DxfViewer.vue'
import StlViewer from './StlViewer.vue'
import StepViewer from './StepViewer.vue'
import ThreeMfViewer from './ThreeMfViewer.vue'
import PdfViewer from './PdfViewer.vue'
import ImageViewer from './ImageViewer.vue'
import TextViewer from './TextViewer.vue'
import ArchiveViewer from './ArchiveViewer.vue'

const props = defineProps<{ file: FileMeta | null; version?: number }>()

const kind = ref<'dxf' | 'stl' | 'step' | 'iges' | '3mf' | 'pdf' | 'image' | 'text' | 'archive' | 'none'>('none')
const dxfText = ref('')
const stlBuffer = ref<ArrayBuffer | null>(null)
const stepBuffer = ref<ArrayBuffer | null>(null)
const threeMfBuffer = ref<ArrayBuffer | null>(null)
const pdfUrl = ref('')
const imgUrl = ref('')
const text = ref('')
const archiveEntries = ref<any[]>([])
const loading = ref(false)
const err = ref('')

let objUrls: string[] = []

function clear() {
  for (const u of objUrls) URL.revokeObjectURL(u)
  objUrls = []
  dxfText.value = ''
  stlBuffer.value = null
  stepBuffer.value = null
  threeMfBuffer.value = null
  pdfUrl.value = ''
  imgUrl.value = ''
  text.value = ''
  archiveEntries.value = []
  err.value = ''
}

async function load() {
  clear()
  kind.value = 'none'
  if (!props.file) return
  const file = props.file
  const ext = file.ext.toLowerCase()
  const ver = props.version

  if (ext === 'dwg' || ext === 'dxf') {
    kind.value = 'dxf'
    loading.value = true
    try {
      const path = ext === 'dwg' ? api.dxfUrl(file.id, ver) : api.previewUrl(file.id, ver)
      const blob = await api.fetchBlob(path)
      dxfText.value = await blob.text()
    } catch (e: any) {
      err.value = (e as ApiError)?.message || String(e)
    } finally {
      loading.value = false
    }
    return
  }

  if (ext === 'stl') {
    kind.value = 'stl'
    loading.value = true
    try {
      const blob = await api.fetchBlob(api.previewUrl(file.id, ver))
      stlBuffer.value = await blob.arrayBuffer()
    } catch (e: any) {
      err.value = (e as ApiError)?.message || String(e)
    } finally {
      loading.value = false
    }
    return
  }

  if (ext === 'step' || ext === 'stp' || ext === 'iges' || ext === 'igs') {
    kind.value = ext === 'iges' || ext === 'igs' ? 'iges' : 'step'
    loading.value = true
    try {
      const blob = await api.fetchBlob(api.previewUrl(file.id, ver))
      stepBuffer.value = await blob.arrayBuffer()
    } catch (e: any) {
      err.value = (e as ApiError)?.message || String(e)
    } finally {
      loading.value = false
    }
    return
  }

  if (ext === '3mf') {
    kind.value = '3mf'
    loading.value = true
    try {
      const blob = await api.fetchBlob(api.previewUrl(file.id, ver))
      threeMfBuffer.value = await blob.arrayBuffer()
    } catch (e: any) {
      err.value = (e as ApiError)?.message || String(e)
    } finally {
      loading.value = false
    }
    return
  }

  if (ext === 'pdf') {
    kind.value = 'pdf'
    loading.value = true
    try {
      const blob = await api.fetchBlob(api.previewUrl(file.id, ver))
      const url = URL.createObjectURL(blob)
      objUrls.push(url)
      pdfUrl.value = url
    } catch (e: any) {
      err.value = (e as ApiError)?.message || String(e)
    } finally {
      loading.value = false
    }
    return
  }

  if (isImage(ext)) {
    kind.value = 'image'
    try {
      const blob = await api.fetchBlob(api.previewUrl(file.id, ver))
      const url = URL.createObjectURL(blob)
      objUrls.push(url)
      imgUrl.value = url
    } catch (e: any) {
      err.value = (e as ApiError)?.message || String(e)
    }
    return
  }

  if (isText(ext)) {
    kind.value = 'text'
    try {
      const blob = await api.fetchBlob(api.previewUrl(file.id, ver))
      text.value = await blob.text()
    } catch (e: any) {
      err.value = (e as ApiError)?.message || String(e)
    }
    return
  }

  if (isArchive(ext)) {
    kind.value = 'archive'
    loading.value = true
    try {
      const resp = await api.request<{ entries: any[] }>(api.archiveListUrl(file.id, ver))
      archiveEntries.value = resp.entries || []
    } catch (e: any) {
      err.value = (e as ApiError)?.message || String(e)
    } finally {
      loading.value = false
    }
    return
  }
}

watch(() => [props.file?.id, props.version], load, { immediate: true })

onBeforeUnmount(clear)
</script>

<template>
  <div class="pane">
    <div v-if="loading" class="hint">加载中…</div>
    <div v-else-if="err" class="hint err">{{ err }}</div>
    <template v-else>
      <DxfViewer v-if="kind === 'dxf' && dxfText" :text="dxfText" />
      <StlViewer v-if="kind === 'stl' && stlBuffer" :buffer="stlBuffer" />
      <StepViewer v-if="(kind === 'step' || kind === 'iges') && stepBuffer" :buffer="stepBuffer" :kind="kind" />
      <ThreeMfViewer v-if="kind === '3mf' && threeMfBuffer" :buffer="threeMfBuffer" />
      <PdfViewer v-if="kind === 'pdf' && pdfUrl" :url="pdfUrl" />
      <ImageViewer v-if="kind === 'image' && imgUrl" :url="imgUrl" :name="file?.name || ''" />
      <TextViewer v-if="kind === 'text' && text !== undefined" :text="text" />
      <ArchiveViewer v-if="kind === 'archive' && archiveEntries.length" :entries="archiveEntries" :file-id="file?.id || 0" :version="version" />
      <div v-if="kind === 'none' && !loading" class="hint">该文件类型暂不支持预览，可直接下载</div>
    </template>
  </div>
</template>

<style scoped>
.pane {
  width: 100%;
  height: 100%;
}
.hint {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--text-dim);
}
.hint.err {
  color: var(--danger);
  padding: 20px;
  text-align: center;
}
</style>
