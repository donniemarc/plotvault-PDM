<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import * as THREE from 'three'
import { createViewer, getModelColor } from './three'
import { onThemeChanged } from '../theme'
import { STEP_CONVERT_URL } from '../api'

const props = defineProps<{ buffer: ArrayBuffer; kind: 'step' | 'iges' }>()

const container = ref<HTMLDivElement>()
const loading = ref(true)
const err = ref('')

let v: ReturnType<typeof createViewer> | null = null
let meshes: THREE.Mesh[] = []
let unlisten: (() => void) | null = null

declare global {
  interface Window {
    occtimportjs?: (opts?: Record<string, unknown>) => Promise<any>
  }
}

/** occt-import-js 在 Web Worker 中解析，避免大模型冻结 UI；失败时回退主线程 */
function readModelInWorker(buffer: Uint8Array, kind: 'step' | 'iges'): Promise<any> {
  return new Promise((resolve, reject) => {
    const base = import.meta.env.BASE_URL
    let worker: Worker
    try {
      worker = new Worker(`${base}vendor/occt/occt-import-js-worker.js`)
    } catch (e: any) {
      reject(e)
      return
    }
    worker.onmessage = (ev) => {
      worker.terminate()
      resolve(ev.data)
    }
    worker.onerror = (e) => {
      worker.terminate()
      reject(new Error(e.message || '解析进程出错'))
    }
    worker.postMessage({ format: kind, buffer, params: null })
  })
}

async function readModelMain(buffer: Uint8Array, kind: 'step' | 'iges'): Promise<any> {
  const base = import.meta.env.BASE_URL
  const options = { locateFile: () => `${base}vendor/occt/occt-import-js.wasm` }
  if (!window.occtimportjs) {
    await new Promise<void>((resolve, reject) => {
      const script = document.createElement('script')
      script.src = `${base}vendor/occt/occt-import-js.js`
      script.onload = () => resolve()
      script.onerror = () => reject(new Error('加载 occt-import-js 脚本失败'))
      document.head.appendChild(script)
    })
  }
  const occt = await window.occtimportjs!(options)
  return kind === 'iges' ? occt.ReadIgesFile(buffer, null) : occt.ReadStepFile(buffer, null)
}

function buildMesh(view: ReturnType<typeof createViewer>, geometryMesh: any) {
  const geometry = new THREE.BufferGeometry()
  geometry.setAttribute('position', new THREE.Float32BufferAttribute(geometryMesh.attributes.position.array, 3))
  if (geometryMesh.attributes.normal) {
    geometry.setAttribute('normal', new THREE.Float32BufferAttribute(geometryMesh.attributes.normal.array, 3))
  }
  const index = Uint32Array.from(geometryMesh.index.array)
  geometry.setIndex(new THREE.BufferAttribute(index, 1))

  const materials: THREE.Material[] = []
  const defaultColor = geometryMesh.color
    ? new THREE.Color(geometryMesh.color[0], geometryMesh.color[1], geometryMesh.color[2])
    : new THREE.Color(getModelColor())
  const baseMat = new THREE.MeshStandardMaterial({ color: defaultColor, metalness: 0.2, roughness: 0.55, flatShading: false })
  baseMat.userData.isDefault = !geometryMesh.color
  materials.push(baseMat)

  if (geometryMesh.brep_faces && geometryMesh.brep_faces.length > 0) {
    for (const face of geometryMesh.brep_faces) {
      const c = face.color ? new THREE.Color(face.color[0], face.color[1], face.color[2]) : defaultColor
      const m = new THREE.MeshStandardMaterial({ color: c, metalness: 0.2, roughness: 0.55 })
      m.userData.isDefault = !face.color
      materials.push(m)
    }
    const triangleCount = geometryMesh.index.array.length / 3
    let triangleIndex = 0
    let faceGroupIndex = 0
    while (triangleIndex < triangleCount) {
      const firstIndex = triangleIndex
      let lastIndex = triangleCount
      let materialIndex = 0
      if (faceGroupIndex < geometryMesh.brep_faces.length && triangleIndex >= geometryMesh.brep_faces[faceGroupIndex].first) {
        lastIndex = geometryMesh.brep_faces[faceGroupIndex].last + 1
        materialIndex = faceGroupIndex + 1
        faceGroupIndex++
      }
      geometry.addGroup(firstIndex * 3, (lastIndex - firstIndex) * 3, materialIndex)
      triangleIndex = lastIndex
    }
  }

  const mesh = new THREE.Mesh(geometry, materials.length > 1 ? materials : materials[0])
  mesh.name = geometryMesh.name || 'part'
  return mesh
}

/** 服务器端转换（需部署 converter 容器并配置 STEP_CONVERT_URL） */
async function readModelOnServer(buffer: Uint8Array, kind: 'step' | 'iges'): Promise<any> {
  const url = `${STEP_CONVERT_URL.replace(/\/+$/, '')}/convert/${kind}`
  const resp = await fetch(url, { method: 'POST', body: buffer.buffer as ArrayBuffer })
  if (!resp.ok) throw new Error(`转换服务返回 HTTP ${resp.status}`)
  return resp.json()
}

onMounted(async () => {
  if (!container.value) return
  v = createViewer(container.value)
  try {
    const buffer = new Uint8Array(props.buffer)
    let result: any
    if (STEP_CONVERT_URL) {
      try {
        result = await readModelOnServer(buffer, props.kind)
      } catch (e: any) {
        console.warn('服务器转换失败，回退本机解析:', e?.message || e)
        result = await readModelInWorker(buffer, props.kind)
      }
    } else {
      try {
        result = await readModelInWorker(buffer, props.kind)
      } catch {
        result = await readModelMain(buffer, props.kind)
      }
    }
    if (!result.meshes || result.meshes.length === 0) {
      err.value = '模型解析成功但未包含几何体'
      return
    }
    for (const gm of result.meshes) {
      const mesh = buildMesh(v, gm)
      meshes.push(mesh)
      v.group.add(mesh)
    }
    v.fit()
    // 主题切换：默认色（未带颜色的面）改随 --viewer-model，不重置相机姿态
    unlisten = onThemeChanged(() => {
      const color = new THREE.Color(getModelColor())
      for (const m of meshes) {
        const mats = Array.isArray(m.material) ? m.material : [m.material]
        for (const mat of mats) {
          if (mat.userData.isDefault) (mat as THREE.MeshStandardMaterial).color.set(color)
        }
      }
    })
  } catch (e: any) {
    err.value = `解析失败: ${e?.message || e}`
  } finally {
    loading.value = false
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
    <div v-if="loading" class="viewer-loading">正在解析模型…</div>
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
.viewer-loading {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-dim);
  background: var(--viewer-veil);
}
.viewer-error {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0 20px;
  text-align: center;
  color: var(--danger);
  background: var(--viewer-veil);
}
</style>