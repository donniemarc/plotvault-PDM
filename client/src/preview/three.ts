import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { getViewerBg, onThemeChanged } from '../theme'
import type { ResolvedTheme } from '../theme'

export interface Viewer3D {
  renderer: THREE.WebGLRenderer
  scene: THREE.Scene
  camera: THREE.PerspectiveCamera
  controls: OrbitControls
  group: THREE.Group
  rafId: number
  fit: () => void
  dispose: () => void
}

// 当前活动预览器注册表：供预览面板工具栏调用（适应窗口/放大/缩小）
let activeViewer: Viewer3D | null = null

export function registerViewer(v: Viewer3D | null) {
  activeViewer = v
}

export function fitActiveViewer() {
  activeViewer?.fit()
}

export function zoomActiveViewer(factor: number) {
  const v = activeViewer
  if (!v) return
  const dir = v.camera.position.clone().sub(v.controls.target)
  v.camera.position.copy(v.controls.target).add(dir.multiplyScalar(factor))
  v.controls.update()
  v.renderer.render(v.scene, v.camera)
}

/** 绝对缩放：基于 fit() 状态的缩放比例（0.25=25%, 0.5=50%, 0.85=85%） */
export function setZoom(ratio: number) {
  const v = activeViewer
  if (!v) return
  
  // 先执行 fit() 获取初始状态
  v.fit()
  
  // 计算初始相机到目标的距离
  const initialDir = v.camera.position.clone().sub(v.controls.target)
  const initialDist = initialDir.length()
  
  // 根据比例计算新的距离
  const newDist = initialDist * ratio
  
  // 保持方向不变，调整距离
  const newDir = initialDir.normalize().multiplyScalar(newDist)
  v.camera.position.copy(v.controls.target).add(newDir)
  
  v.controls.update()
  v.renderer.render(v.scene, v.camera)
}

/** 白天主题 ACI 近白重映射表（UI-design-spec §3.7）；晚上保持现值 */
const LIGHT_ACI: Record<number, number> = {
  7: 0x2a3442,
  8: 0x6b7886,
  9: 0x3a4553,
  30: 0x4f5a66,
  31: 0x6b7886,
  250: 0x4f5a66,
  255: 0x3a4553,
  256: 0x2a3442,
}

/** 按主题重映射 ACI 线色（非近白系原样返回） */
export function mapDxfColorForTheme(idx: number, color: number, theme: ResolvedTheme): number {
  if (theme !== 'light') return color
  return LIGHT_ACI[idx] ?? color
}

/** 读取当前主题下的 3D 默认材质色（--viewer-model） */
export function getModelColor(): string {
  try {
    const v = getComputedStyle(document.documentElement)
      .getPropertyValue('--viewer-model')
      .trim()
    if (v) return v
  } catch {
    /* ignore */
  }
  return '#8fb7e0'
}

export function createViewer(
  container: HTMLElement,
  options?: { background?: string },
): Viewer3D {
  const width = container.clientWidth || 600
  const height = container.clientHeight || 400

  const scene = new THREE.Scene()
  scene.background = new THREE.Color(options?.background ?? getViewerBg())

  const camera = new THREE.PerspectiveCamera(50, width / height, 0.001, 100000)
  camera.position.set(1, -1, 1)

  const renderer = new THREE.WebGLRenderer({ antialias: true })
  renderer.setSize(width, height)
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2))
  container.appendChild(renderer.domElement)

  const controls = new OrbitControls(camera, renderer.domElement)
  controls.target.set(0, 0, 0)
  controls.update()

  // 灯光：修复 MeshStandardMaterial 无光照黑屏（STL/STEP 全黑问题）
  const ambient = new THREE.AmbientLight(0xffffff, 0.6)
  scene.add(ambient)
  const sun = new THREE.DirectionalLight(0xffffff, 1.2)
  sun.position.set(1, -1, 1)
  scene.add(sun)
  scene.add(sun.target)

  // 方向光随 fit 相机朝向：保证首次正视模型时受光面朝向观察者
  function syncSun() {
    sun.position.copy(camera.position)
    sun.target.position.copy(controls.target)
    sun.target.updateMatrixWorld()
  }

  const group = new THREE.Group()
  scene.add(group)

  const viewer = {} as Viewer3D
  viewer.group = group
  registerViewer(viewer)

  viewer.rafId = requestAnimationFrame(function loop() {
    controls.update()
    renderer.render(scene, camera)
    viewer.rafId = requestAnimationFrame(loop)
  })

  viewer.fit = () => {
    const box = new THREE.Box3().setFromObject(group)
    const size = box.getSize(new THREE.Vector3())
    const center = box.getCenter(new THREE.Vector3())
    const maxDim = Math.max(size.x, size.y, size.z, 1e-6)
    const dist = maxDim * 2.4
    camera.position.copy(center).add(new THREE.Vector3(dist * 0.9, -dist * 0.7, dist * 0.9))
    controls.target.copy(center)
    controls.update()
    syncSun()
    renderer.render(scene, camera)
  }

  // 主题切换：即时更新画布背景，不重置相机姿态
  const unlisten = onThemeChanged(() => {
    scene.background = new THREE.Color(getViewerBg())
    renderer.render(scene, camera)
  })

  // 容器尺寸变化（面板开合/窗口缩放）时同步渲染尺寸
  let ro: ResizeObserver | null = null
  if (typeof ResizeObserver !== 'undefined') {
    ro = new ResizeObserver(() => {
      const w = container.clientWidth
      const h = container.clientHeight
      if (!w || !h) return
      camera.aspect = w / h
      camera.updateProjectionMatrix()
      renderer.setSize(w, h)
      renderer.render(scene, camera)
    })
    ro.observe(container)
  }

  viewer.dispose = () => {
    if (activeViewer === viewer) registerViewer(null)
    unlisten()
    ro?.disconnect()
    cancelAnimationFrame(viewer.rafId)
    controls.dispose()
    renderer.dispose()
    renderer.domElement.remove()
  }

  return viewer
}