<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import * as THREE from 'three'
import { STLLoader } from 'three/examples/jsm/loaders/STLLoader.js'
import { createViewer, getModelColor } from './three'
import { onThemeChanged } from '../theme'

const props = defineProps<{ buffer: ArrayBuffer }>()

const container = ref<HTMLDivElement>()
const err = ref('')

let v: ReturnType<typeof createViewer> | null = null
let mesh: THREE.Mesh | null = null
let unlisten: (() => void) | null = null

onMounted(() => {
  if (!container.value) return
  v = createViewer(container.value)
  try {
    const loader = new STLLoader()
    const geometry = loader.parse(props.buffer)
    geometry.computeVertexNormals()
    mesh = new THREE.Mesh(
      geometry,
      new THREE.MeshStandardMaterial({ color: getModelColor(), metalness: 0.15, roughness: 0.55, flatShading: true }),
    )
    v.group.add(mesh)
    v.fit()
    // 主题切换：仅改材质色，不重置相机姿态
    unlisten = onThemeChanged(() => {
      if (mesh) (mesh.material as THREE.MeshStandardMaterial).color.set(getModelColor())
    })
  } catch (e: any) {
    err.value = `STL 解析出错: ${e?.message || e}`
  }
})

onBeforeUnmount(() => {
  unlisten?.()
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