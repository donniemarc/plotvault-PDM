<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import * as THREE from 'three'
import { ThreeMFLoader } from 'three/examples/jsm/loaders/3MFLoader.js'
import { createViewer } from './three'

const props = defineProps<{ buffer: ArrayBuffer }>()

const container = ref<HTMLDivElement>()
const err = ref('')

let v: ReturnType<typeof createViewer> | null = null

onMounted(() => {
  if (!container.value) return
  v = createViewer(container.value)
  try {
    const loader = new ThreeMFLoader()
    const group = loader.parse(props.buffer)
    v.group.add(group)
    v.fit()
  } catch (e: any) {
    err.value = `3MF 解析出错: ${e?.message || e}`
  }
})

onBeforeUnmount(() => {
  v?.dispose()
  v = null
})
</script>

<template>
  <div class="viewer-wrap">
    <div ref="container" class="viewer-canvas"></div>
    <div v-if="err" class="viewer-error">{{ err }}</div>
  </div>
</template>

<style scoped>
.viewer-wrap {
  position: relative;
  width: 100%;
  height: 100%;
}
.viewer-canvas {
  width: 100%;
  height: 100%;
}
.viewer-error {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--danger);
  background: var(--viewer-veil);
}
</style>