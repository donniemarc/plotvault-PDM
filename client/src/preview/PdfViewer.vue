<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import * as pdfjsLib from 'pdfjs-dist'
import workerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url'

const props = defineProps<{ url: string }>()

const container = ref<HTMLDivElement>()
const pages: HTMLCanvasElement[] = []
const loading = ref(true)
const err = ref('')

pdfjsLib.GlobalWorkerOptions.workerSrc = workerUrl

onMounted(async () => {
  try {
    const doc = await pdfjsLib.getDocument({ url: props.url }).promise
    for (let i = 1; i <= doc.numPages; i++) {
      const page = await doc.getPage(i)
      const viewport = page.getViewport({ scale: 1.4 })
      const canvas = document.createElement('canvas')
      canvas.width = viewport.width
      canvas.height = viewport.height
      canvas.style.width = '100%'
      canvas.style.height = 'auto'
      canvas.style.display = 'block'
      canvas.style.margin = '0 auto 12px'
      container.value?.appendChild(canvas)
      pages.push(canvas)
      await page.render({ canvas, viewport }).promise
    }
    loading.value = false
  } catch (e: any) {
    err.value = `PDF 渲染失败: ${e?.message || e}`
    loading.value = false
  }
})

onBeforeUnmount(() => {
  for (const c of pages) c.remove()
})
</script>

<template>
  <div class="pdf-wrap">
    <div v-if="loading" class="pdf-loading">正在加载 PDF…</div>
    <div v-if="err" class="pdf-error">{{ err }}</div>
    <div ref="container" class="pdf-pages"></div>
  </div>
</template>

<style scoped>
.pdf-wrap {
  width: 100%;
  height: 100%;
  overflow: auto;
  padding: 16px;
  background: var(--viewer-bg);
}
.pdf-loading {
  position: sticky;
  top: 0;
  text-align: center;
  color: var(--text-dim);
  padding: 12px;
}
.pdf-error {
  position: sticky;
  top: 0;
  text-align: center;
  color: var(--danger);
  padding: 12px;
}
.pdf-pages {
  width: 100%;
}
</style>
