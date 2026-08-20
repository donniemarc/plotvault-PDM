<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import * as THREE from 'three'
import DxfParser from 'dxf-parser'
import { createViewer, mapDxfColorForTheme } from './three'
import { getResolvedTheme, onThemeChanged } from '../theme'

const props = defineProps<{ text: string }>()

const container = ref<HTMLDivElement>()
const err = ref('')

let v: ReturnType<typeof createViewer> | null = null
let dxfDoc: any = null
let blocksMap: Map<string, any> | null = null
let unlisten: (() => void) | null = null

const ACI: Record<number, number> = {
  1: 0xff5050, 2: 0xffd53f, 3: 0x4fbf6f, 4: 0x3fd6c2, 5: 0x4f9fff, 6: 0xc04fff,
  7: 0xe8ecf1, 8: 0x9aa7b5, 9: 0xd8dee6, 10: 0xff8080, 11: 0xffe080, 12: 0x80d6a0,
  13: 0x80e8da, 14: 0x80bcff, 15: 0xdc80ff, 30: 0x4f4f4f, 31: 0x9f9f9f,
  40: 0x5f4040, 41: 0x9f7f7f, 42: 0x3f2f2f, 43: 0x7f5f5f, 44: 0xbf9f9f,
  50: 0x9fbf9f, 60: 0x4f4f3f, 61: 0x9f9f7f, 63: 0x7f7f5f, 64: 0xbfbf9f,
  70: 0x9fbfbf, 250: 0xbfbfbf, 255: 0x9f9f9f,
  256: 0xe8ecf1,
}

/** 白天主题 ACI 近白重映射在 three.ts（mapDxfColorForTheme） */

function resolveColor(entity: any, dxf: any): number {
  let idx = entity.colorIndex ?? entity.color ?? 7
  if (idx === 256) {
    const layer = dxf?.tables?.layer?.layers?.[entity.layer]
    if (layer && layer.colorIndex) idx = layer.colorIndex
  }
  const known = ACI[idx]
  if (known !== undefined) return mapDxfColorForTheme(idx, known, getResolvedTheme())
  // fallback：默认近白，白天主题映射为深墨蓝灰
  return mapDxfColorForTheme(7, 0xe8ecf1, getResolvedTheme())
}

function colorMat(entity: any, dxf: any): THREE.LineBasicMaterial {
  return new THREE.LineBasicMaterial({ color: resolveColor(entity, dxf) })
}

function addLine(g: THREE.BufferGeometry, mat: THREE.Material) {
  v?.group.add(new THREE.Line(g, mat))
}

function transformPoint(p: any, ip: { x: number; y: number; z: number }, scale: { x: number; y: number; z: number }, cos: number, sin: number) {
  const sx = p.x * scale.x
  const sy = p.y * scale.y
  const sz = (p.z ?? 0) * (scale.z ?? 1)
  return {
    x: sx * cos - sy * sin + ip.x,
    y: sx * sin + sy * cos + ip.y,
    z: sz + (ip.z ?? 0),
  }
}

function transformEntity(e: any, ip: { x: number; y: number; z: number }, scale: { x: number; y: number; z: number }, cos: number, sin: number): any {
  const out = { ...e }
  const tf = (p: any) => transformPoint(p, ip, scale, cos, sin)
  const rotDeg = (Math.atan2(sin, cos) * 180) / Math.PI
  switch (e.type) {
    case 'LINE':
      out.vertices = (e.vertices || []).map(tf)
      break
    case 'LWPOLYLINE':
    case 'POLYLINE':
      out.vertices = (e.vertices || []).map(tf)
      break
    case 'SPLINE':
      out.controlPoints = (e.controlPoints || []).map(tf)
      out.fitPoints = (e.fitPoints || []).map(tf)
      break
    case 'CIRCLE':
      out.center = tf(e.center)
      out.radius = e.radius * (Math.abs(scale.x) + Math.abs(scale.y)) / 2
      break
    case 'ARC':
      out.center = tf(e.center)
      out.radius = e.radius * (Math.abs(scale.x) + Math.abs(scale.y)) / 2
      out.startAngle = e.startAngle + rotDeg
      out.endAngle = e.endAngle + rotDeg
      break
    case 'ELLIPSE':
      out.center = tf(e.center)
      out.majorAxisEndPoint = {
        x: e.majorAxisEndPoint.x * scale.x,
        y: e.majorAxisEndPoint.y * scale.y,
        z: (e.majorAxisEndPoint.z ?? 0) * (scale.z ?? 1),
      }
      out.startAngle = e.startAngle + rotDeg / 360
      out.endAngle = e.endAngle + rotDeg / 360
      break
    case 'POINT':
    case 'TEXT':
    case 'MTEXT':
      out.position = tf(e.position)
      break
  }
  return out
}

function handleEntity(e: any, dxf: any, blocks?: Map<string, any> | null, depth = 0) {
  if (depth > 12) return
  if (e.type === 'INSERT') {
    const blk = blocks?.get(e.block)
    if (!blk || !blk.entities) return
    const ip = e.position || { x: 0, y: 0, z: 0 }
    const scale = e.scale || { x: 1, y: 1, z: 1 }
    const rot = ((e.rotation || 0) * Math.PI) / 180
    const cos = Math.cos(rot)
    const sin = Math.sin(rot)
    for (const sub of blk.entities) {
      handleEntity(transformEntity(sub, ip, scale, cos, sin), dxf, blocks, depth + 1)
    }
    return
  }
  const mat = colorMat(e, dxf)
  switch (e.type) {
    case 'LINE': {
      const g = new THREE.BufferGeometry().setFromPoints([
        new THREE.Vector3(e.vertices?.[0]?.x ?? 0, e.vertices?.[0]?.y ?? 0, e.vertices?.[0]?.z ?? 0),
        new THREE.Vector3(e.vertices?.[1]?.x ?? 0, e.vertices?.[1]?.y ?? 0, e.vertices?.[1]?.z ?? 0),
      ])
      addLine(g, mat)
      break
    }
    case 'LWPOLYLINE':
    case 'POLYLINE': {
      const pts = (e.vertices || []).map((p: any) => new THREE.Vector3(p.x ?? 0, p.y ?? 0, p.z ?? 0))
      if (pts.length < 2) break
      const closed = !!(e.closed || e.shape)
      const g = new THREE.BufferGeometry().setFromPoints(closed ? [...pts, pts[0]] : pts)
      addLine(g, mat)
      break
    }
    case 'CIRCLE': {
      const pts: THREE.Vector3[] = []
      const segs = 64
      for (let i = 0; i <= segs; i++) {
        const a = (i / segs) * Math.PI * 2
        pts.push(new THREE.Vector3(e.center.x + Math.cos(a) * e.radius, e.center.y + Math.sin(a) * e.radius, e.center.z || 0))
      }
      const g = new THREE.BufferGeometry().setFromPoints(pts)
      addLine(g, mat)
      break
    }
    case 'ARC': {
      const pts: THREE.Vector3[] = []
      const segs = 48
      const a0 = (e.startAngle * Math.PI) / 180
      const a1 = (e.endAngle * Math.PI) / 180
      for (let i = 0; i <= segs; i++) {
        const a = a0 + ((a1 - a0) * i) / segs
        pts.push(new THREE.Vector3(e.center.x + Math.cos(a) * e.radius, e.center.y + Math.sin(a) * e.radius, e.center.z || 0))
      }
      const g = new THREE.BufferGeometry().setFromPoints(pts)
      addLine(g, mat)
      break
    }
    case 'ELLIPSE': {
      const pts: THREE.Vector3[] = []
      const segs = 64
      const rx = Math.hypot(e.majorAxisEndPoint.x, e.majorAxisEndPoint.y)
      const angle = Math.atan2(e.majorAxisEndPoint.y, e.majorAxisEndPoint.x)
      const a0 = e.startAngle * Math.PI
      const a1 = e.endAngle * Math.PI
      for (let i = 0; i <= segs; i++) {
        const t = a0 + ((a1 - a0) * i) / segs
        const x = rx * Math.cos(t)
        const y = rx * (e.axisRatio || 1) * Math.sin(t)
        pts.push(new THREE.Vector3(e.center.x + x * Math.cos(angle) - y * Math.sin(angle), e.center.y + x * Math.sin(angle) + y * Math.cos(angle), e.center.z || 0))
      }
      const g = new THREE.BufferGeometry().setFromPoints(pts)
      addLine(g, mat)
      break
    }
    case 'SPLINE': {
      const pts = (e.controlPoints || e.fitPoints || []).map((p: any) => new THREE.Vector3(p.x ?? 0, p.y ?? 0, p.z ?? 0))
      if (pts.length < 2) break
      const g = new THREE.BufferGeometry().setFromPoints(pts)
      addLine(g, mat)
      break
    }
    case 'POINT': {
      const g = new THREE.BufferGeometry()
      g.setAttribute('position', new THREE.Float32BufferAttribute([e.position.x, e.position.y, e.position.z || 0], 3))
      v?.group.add(new THREE.Points(g, new THREE.PointsMaterial({ color: resolveColor(e, dxf), sizeAttenuation: false })))
      break
    }
    case 'TEXT':
    case 'MTEXT': {
      const g = new THREE.BufferGeometry()
      g.setAttribute('position', new THREE.Float32BufferAttribute([e.position?.x ?? 0, e.position?.y ?? 0, e.position?.z ?? 0], 3))
      v?.group.add(new THREE.Points(g, new THREE.PointsMaterial({ color: resolveColor(e, dxf), sizeAttenuation: false })))
      break
    }
    default:
      break
  }
}

function clearGroup() {
  if (!v) return
  for (let i = v.group.children.length - 1; i >= 0; i--) {
    const c = v.group.children[i]
    if (c instanceof THREE.Mesh || c instanceof THREE.Line || c instanceof THREE.Points) {
      c.geometry?.dispose()
      const m = (c as THREE.Line | THREE.Points).material
      if (Array.isArray(m)) m.forEach((mm) => mm.dispose())
      else m?.dispose()
    }
    v.group.remove(c)
  }
}

/** 重新渲染整个 DXF 场景（主题切换时重涂线色）；几何不变，不重置相机姿态 */
function renderDxf() {
  if (!v || !dxfDoc) return
  clearGroup()
  for (const e of dxfDoc.entities) handleEntity(e, dxfDoc, blocksMap)
}

onMounted(() => {
  if (!container.value) return
  v = createViewer(container.value)
  try {
    const parser = new DxfParser()
    const dxf = parser.parseSync(props.text)
    if (!dxf) {
      err.value = 'DXF 解析失败'
      return
    }
    dxfDoc = dxf
    const blocks = new Map<string, any>()
    const rawBlocks = (dxf.blocks || {}) as Record<string, any>
    for (const name of Object.keys(rawBlocks)) blocks.set(name, rawBlocks[name])
    blocksMap = blocks
    for (const e of dxf.entities) handleEntity(e, dxfDoc, blocksMap)
    v.fit()
    if (v.group.children.length === 0) {
      err.value = '该图纸没有可渲染的几何实体（块引用已展开）'
    }
    unlisten = onThemeChanged(() => {
      if (dxfDoc && v) renderDxf()
    })
  } catch (e: any) {
    err.value = `DXF 解析出错: ${e?.message || e}`
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