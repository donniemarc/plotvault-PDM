<script setup lang="ts">
import { computed, inject, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import { api } from '../api'
import type { FileMeta, Folder } from '../types'
import { getConfig } from '../api'
import { saveBlob, getFilesFromDropEvent } from '../utils'
import FolderTree from '../components/FolderTree.vue'
import FileList from '../components/FileList.vue'
import UploadDialog from '../components/UploadDialog.vue'
import VersionPanel from '../components/VersionPanel.vue'
import PropertyPanel from '../components/PropertyPanel.vue'
import PreviewPane from '../preview/PreviewPane.vue'
import { fitActiveViewer, zoomActiveViewer } from '../preview/three'
import JSZip from 'jszip'

const toast = inject<(msg: string, type?: string) => void>('toast') || (() => {})

const folders = ref<Folder[]>([])
const files = ref<FileMeta[]>([])
const loading = ref(false)
const selectedFolder = ref<number | null>(null)

const ROOT_KEY = 'plotvault_pdm_root_name'
const rootName = ref(localStorage.getItem(ROOT_KEY) || '全部文件')
function setRootName(name: string) {
  const n = name.trim()
  if (!n) return
  rootName.value = n
  localStorage.setItem(ROOT_KEY, n)
}

const currentFolderName = computed(() => {
  if (selectedFolder.value == null) return rootName.value
  return folders.value.find((f) => f.id === selectedFolder.value)?.name || rootName.value
})

const previewFile = ref<FileMeta | null>(null)
const previewVersion = ref<number | undefined>(undefined)
const versionPanelFile = ref<FileMeta | null>(null)

// 属性面板状态
const activePropertyTab = ref<'property' | 'filelist' | 'versions' | 'related' | 'history'>('property')
const selectedFolderForProps = ref<Folder | null>(null)
const selectedFileForProps = ref<FileMeta | null>(null)
const activePreviewingId = computed(() => selectedFileForProps.value?.id ?? previewFile.value?.id ?? null)

const searchOpen = ref(false)
const searchQ = ref('')
const searchResults = ref<FileMeta[]>([])
const searching = ref(false)

const uploadOpen = ref(false)
const droppedFiles = ref<File[]>([])
const newFolderOpen = ref(false)
const newFolderName = ref('')
const newFolderParent = ref<number | null>(null)

const renameModal = ref<{ type: 'file' | 'folder' | 'root'; id: number; name: string } | null>(null)
const renameName = ref('')

const moveModal = ref<{ files: FileMeta[] } | null>(null)
const moveTarget = ref<number | 'root' | null>(null)

const newFolderInput = ref<HTMLInputElement | null>(null)
const renameInput = ref<HTMLInputElement | null>(null)

// 移动弹窗：按层级展开的文件夹列表（用于缩进展示层级关系）
const moveFolderRows = computed<{ folder: Folder; depth: number }[]>(() => {
  const children = new Map<number, Folder[]>()
  for (const f of folders.value) {
    const key = f.parent_id ?? 0
    if (!children.has(key)) children.set(key, [])
    children.get(key)!.push(f)
  }
  const out: { folder: Folder; depth: number }[] = []
  const walk = (pid: number, depth: number) => {
    const list = children.get(pid) || []
    list.forEach((f) => {
      out.push({ folder: f, depth })
      walk(f.id, depth + 1)
    })
  }
  walk(0, 0)
  return out
})

// 批量选择（复选框）
const selectedIds = ref<Set<number>>(new Set())
function clearSelection() {
  selectedIds.value = new Set()
}
function toggleSelect(id: number) {
  const s = new Set(selectedIds.value)
  if (s.has(id)) s.delete(id)
  else s.add(id)
  selectedIds.value = s
}
function toggleSelectAll(checked: boolean) {
  selectedIds.value = checked ? new Set(visibleFiles.value.map((f) => f.id)) : new Set()
}
const allSelected = computed(
  () => visibleFiles.value.length > 0 && visibleFiles.value.every((f) => selectedIds.value.has(f.id)),
)
const someSelected = computed(() => visibleFiles.value.some((f) => selectedIds.value.has(f.id)))
const filesById = computed(() => new Map(files.value.map((f) => [f.id, f])))

// 删除确认（应用内主题化模态框，替代原生 confirm）
const confirmModal = ref<{ type: 'file' | 'folder' | 'files'; id: number; name: string; ids?: number[] } | null>(null)

const dragOver = ref(false)

const treeRef = ref<InstanceType<typeof FolderTree> | null>(null)

// 同步状态
const syncStatus = ref<{ is_syncing: boolean; last_sync_secs: number | null; last_sync_result: string | null }>({
  is_syncing: false,
  last_sync_secs: null,
  last_sync_result: null,
})
let syncPollTimer: ReturnType<typeof setInterval> | null = null

async function pollSyncStatus() {
  try {
    const s = await api.getSyncStatus()
    syncStatus.value = s
    // 如果刚完成同步且之前在同步中，刷新文件列表
    if (!s.is_syncing && s.last_sync_secs !== null && s.last_sync_secs < 2) {
      await load()
    }
  } catch {
    // 忽略错误
  }
}

// ESC 键关闭预览
function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    if (previewFile.value) { previewFile.value = null; return }
    if (versionPanelFile.value) { versionPanelFile.value = null; return }
  }
}

// ---------- 文件树侧栏：可拖拽调宽 ----------
const sidebarW = ref<number>(Number(localStorage.getItem('plotvault_pdm_sidebar_w')) || 240)

function clampSidebarW(w: number): number {
  const MIN_SIDEBAR = 180
  const MAX_SIDEBAR = Math.min(800, window.innerWidth - 400)
  return Math.min(Math.max(Math.round(w), MIN_SIDEBAR), Math.round(MAX_SIDEBAR))
}

function onSidebarResizeStart(e: MouseEvent) {
  e.preventDefault()
  const startX = e.clientX
  const startW = sidebarW.value
  function onMove(ev: MouseEvent) {
    sidebarW.value = clampSidebarW(startW + (ev.clientX - startX))
    localStorage.setItem('plotvault_pdm_sidebar_w', String(sidebarW.value))
  }
  function onUp() {
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
    document.body.classList.remove('resizing-cursor')
  }
  document.body.classList.add('resizing-cursor')
  window.addEventListener('mousemove', onMove)
  window.addEventListener('mouseup', onUp)
}

// ---------- 预览面板：可拖拽调宽 / 全屏 ----------
const previewFullscreen = ref(false)
const rightPanelWidth = ref<number>(Number(localStorage.getItem('plotvault_pdm_panel_w')) || 460)

// 面板最大宽度 = 窗口宽度 - 侧栏 - 分隔条 - 主区最小宽度（避免面板挤压/盖住文件列表）
// MIN_MAIN 需 ≥ 固定列总宽(622px) + 名称列最小宽度(140px) = 762px，确保文件名不被挤压消失
function clampPanelW(w: number): number {
  const MIN_MAIN = 500
  const maxW = Math.max(320, window.innerWidth - sidebarW.value - 6 - MIN_MAIN)
  return Math.min(Math.max(Math.round(w), 320), Math.round(maxW))
}

// 主区可用宽度（窗口 - 侧栏 - 分隔条 - 预览面板）
function mainAreaWidth(): number {
  return window.innerWidth - sidebarW.value - 6 - rightPanelWidth.value
}

// 面板过宽导致主区变窄时，逐步收起次要按钮，避免头部溢出被面板盖住
const headTight = computed(() => {
  if (!previewFile.value && !versionPanelFile.value) return false
  return mainAreaWidth() <= 520
})
const headVeryTight = computed(() => {
  if (!previewFile.value && !versionPanelFile.value) return false
  return mainAreaWidth() <= 420
})

function onResizeStart(e: MouseEvent) {
  e.preventDefault()
  const startX = e.clientX
  const startW = rightPanelWidth.value
  function onMove(ev: MouseEvent) {
    rightPanelWidth.value = clampPanelW(startW - (ev.clientX - startX))
    localStorage.setItem('plotvault_pdm_panel_w', String(rightPanelWidth.value))
  }
  function onUp() {
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
    document.body.classList.remove('resizing-cursor')
  }
  document.body.classList.add('resizing-cursor')
  window.addEventListener('mousemove', onMove)
  window.addEventListener('mouseup', onUp)
}

function toggleFullscreen() {
  previewFullscreen.value = !previewFullscreen.value
}

async function load() {
  loading.value = true
  try {
    const t = await api.tree()
    folders.value = t.folders
    files.value = t.files
    // 剔除已被删除的文件 ID，保持选中状态有效
    const alive = new Set(t.files.map((f) => f.id))
    selectedIds.value = new Set(Array.from(selectedIds.value).filter((id) => alive.has(id)))
  } catch (e: any) {
    toast(e?.message || '加载失败', 'error')
  } finally {
    loading.value = false
  }
}

const visibleFiles = computed(() => {
  if (searchOpen.value) return searchResults.value
  return files.value.filter((f) => f.folder_id === selectedFolder.value)
})

// 数据统计（根目录显示）
const stats = computed(() => {
  const fileCount = files.value.length
  const folderCount = folders.value.length
  const totalSize = files.value.reduce((sum, f) => sum + (f.size || 0), 0)
  return { fileCount, folderCount, totalSize }
})

// 最近修改的文件（按更新时间排序，取前5个）
const recentFiles = computed(() => {
  return [...files.value]
    .sort((a, b) => (b.updated_at || '').localeCompare(a.updated_at || ''))
    .slice(0, 5)
})

// 当前目录的直属子文件夹（用于空状态展示子文件夹卡片）
const childFolders = computed(() =>
  folders.value.filter((f) => f.parent_id === selectedFolder.value),
)

// 是否是根目录
const isRoot = computed(() => selectedFolder.value === null && !searchOpen.value)

// 当前目录已有文件名（用于上传时重复检测）
const currentDirNames = computed(() =>
  visibleFiles.value.map((f) => f.name.toLowerCase()),
)

async function selectFolder(id: number | null) {
  selectedFolder.value = id
  searchOpen.value = false
  previewFile.value = null
  versionPanelFile.value = null
  selectedFileForProps.value = null
  
  // 根目录保持原样显示文件列表，不显示属性面板
  if (id === null) {
    selectedFolderForProps.value = null
  } else {
    // 一级目录及以后显示属性面板
    const folder = folders.value.find(f => f.id === id)
    selectedFolderForProps.value = folder || null
  }
  
  activePropertyTab.value = 'property'
  clearSelection()
}

function folderName(id: number | null): string {
  if (id == null) return rootName.value
  return folders.value.find((f) => f.id === id)?.name || rootName.value
}

async function doSearch() {
  const q = searchQ.value.trim()
  if (!q) return
  searching.value = true
  try {
    const r = await api.search(q)
    searchResults.value = r.files
    searchOpen.value = true
    clearSelection()
  } catch (e: any) {
    toast(e?.message || '搜索失败', 'error')
  } finally {
    searching.value = false
  }
}

function clearSearch() {
  searchOpen.value = false
  searchQ.value = ''
  searchResults.value = []
  clearSelection()
}

// ---------- preview / versions ----------
function openPreview(f: FileMeta) {
  previewFile.value = null
  previewVersion.value = undefined
  versionPanelFile.value = null
  
  // 只显示属性面板（内嵌预览），不弹出右侧预览窗口
  selectedFileForProps.value = f
  selectedFolderForProps.value = null
  selectedFolder.value = null
  activePropertyTab.value = 'property'
}

function openVersions(f: FileMeta) {
  versionPanelFile.value = f
  previewFile.value = null
}

async function download(f: FileMeta, version?: number) {
  try {
    const blob = await api.fetchBlob(api.downloadUrl(f.id, version))
    const name = version ? `${f.name.replace(/\.[^.]+$/, '')}_v${version}.${f.ext}` : f.name
    await saveBlob(name, blob)
  } catch (e: any) {
    toast(e?.message || '下载失败', 'error')
  }
}

// ---------- folders ----------
function openNewFolder(parentId: number | null) {
  newFolderParent.value = parentId
  newFolderName.value = ''
  newFolderOpen.value = true
  nextTick(() => { newFolderInput.value?.focus() })
}

async function createFolder() {
  const name = newFolderName.value.trim()
  if (!name) return
  try {
    const folder = await api.createFolder(name, newFolderParent.value)
    newFolderOpen.value = false
    await load()
    // 自动展开根节点与父级，让新建文件夹直接可见
    treeRef.value?.expandTo(folder.id)
  } catch (e: any) {
    toast(e?.message || '创建失败', 'error')
  }
}

function openRename(type: 'file' | 'folder' | 'root', id: number, name: string) {
  renameModal.value = { type, id, name }
  renameName.value = name
  nextTick(() => {
    const el = renameInput.value
    if (el) {
      el.focus()
      const len = el.value.length
      el.setSelectionRange(len, len)
    }
  })
}

async function doRename() {
  if (!renameModal.value) return
  const name = renameName.value.trim()
  if (!name) return
  try {
    if (renameModal.value.type === 'root') {
      setRootName(name)
    } else if (renameModal.value.type === 'file') {
      await api.patchFile(renameModal.value.id, { name })
    } else {
      await api.renameFolder(renameModal.value.id, name)
    }
    renameModal.value = null
    await load()
  } catch (e: any) {
    toast(e?.message || '重命名失败', 'error')
  }
}

function openMove(f: FileMeta) {
  moveModal.value = { files: [f] }
  moveTarget.value = null
}

function openMoveBatch() {
  const items = Array.from(selectedIds.value)
    .map((id) => filesById.value.get(id))
    .filter((f): f is FileMeta => !!f)
  if (!items.length) return
  moveModal.value = { files: items }
  moveTarget.value = null
}

async function doMove() {
  if (!moveModal.value) return
  try {
    const folderId = moveTarget.value === 'root' ? null : moveTarget.value
    for (const f of moveModal.value.files) {
      if (f.folder_id === folderId) continue
      await api.patchFile(f.id, { folder_id: folderId })
    }
    moveModal.value = null
    await load()
    clearSelection()
    toast('已移动', 'ok')
  } catch (e: any) {
    toast(e?.message || '移动失败', 'error')
  }
}

// ---------- 拖拽移动文件夹 ----------
async function onMoveFolder(folderId: number, targetParentId: number | null) {
  try {
    await api.moveFolder(folderId, targetParentId)
    await load()
    toast('已移动', 'ok')
  } catch (e: any) {
    toast(e?.message || '移动失败', 'error')
  }
}

// ---------- 拖拽移动文件 ----------
async function onMoveFiles(fileIds: number[], targetFolderId: number | null) {
  try {
    for (const id of fileIds) {
      const f = filesById.value.get(id)
      if (f && f.folder_id !== targetFolderId) {
        await api.patchFile(id, { folder_id: targetFolderId })
      }
    }
    await load()
    clearSelection()
    toast('已移动', 'ok')
  } catch (e: any) {
    toast(e?.message || '移动失败', 'error')
  }
}

// ---------- 打开文件所在文件夹 ----------
const NAS_ROOT_KEY = 'plotvault_pdm_nas_root'

function getLocalPath(diskPath: string): string {
  const nasRoot = localStorage.getItem(NAS_ROOT_KEY)?.trim() || ''
  const libraryPrefix = '/data/library/'
  if (diskPath.startsWith(libraryPrefix)) {
    const relative = diskPath.slice(libraryPrefix.length).replace(/\//g, '\\')
    return nasRoot.replace(/[\\/]+$/, '') + (relative ? '\\' + relative : '')
  }
  return nasRoot.replace(/[\\/]+$/, '')
}

async function openFileFolder(file: FileMeta) {
  const nasRoot = localStorage.getItem(NAS_ROOT_KEY)?.trim()
  if (!nasRoot) {
    toast('请先在设置中配置「NAS 文件映射路径」', 'error')
    return
  }
  try {
    const resp = await api.getFileDiskPath(file.id)
    const localPath = getLocalPath(resp.path)
    console.log('[openFileFolder] server:', resp.path, '→ local:', localPath)
    const { Command } = await import('@tauri-apps/plugin-shell')
    await Command.create('powershell', ['-Command', `explorer "${localPath}"`]).execute()
  } catch (e: any) {
    console.error('[openFileFolder] error:', e)
    toast(`打开失败: ${e?.message || e}`, 'error')
  }
}

async function openFolderFromTree(folderId: number | null) {
  const nasRoot = localStorage.getItem(NAS_ROOT_KEY)?.trim()
  if (!nasRoot) {
    toast('请先在设置中配置「NAS 文件映射路径」', 'error')
    return
  }
  try {
    if (folderId === null) {
      // 根目录：直接打开映射路径
      const localPath = nasRoot.replace(/[\\/]+$/, '')
      const { Command } = await import('@tauri-apps/plugin-shell')
      await Command.create('powershell', ['-Command', `explorer "${localPath}"`]).execute()
      return
    }
    const resp = await api.getFolderDiskPath(folderId)
    const localPath = getLocalPath(resp.path)
    console.log('[openFolderFromTree] server:', resp.path, '→ local:', localPath)
    const { Command } = await import('@tauri-apps/plugin-shell')
    await Command.create('powershell', ['-Command', `explorer "${localPath}"`]).execute()
  } catch (e: any) {
    console.error('[openFolderFromTree] error:', e)
    toast(`打开失败: ${e?.message || e}`, 'error')
  }
}

function removeFile(f: FileMeta) {
  confirmModal.value = { type: 'file', id: f.id, name: f.name }
}

// 点击空白区域关闭预览窗口
function onContentClick(e: Event) {
  // 只在点击内容区域本身（非文件行）时关闭预览
  const target = e.target as HTMLElement
  if (target.classList.contains('content') || target.classList.contains('list-wrap') || target.classList.contains('empty')) {
    previewFile.value = null
    versionPanelFile.value = null
  }
}

async function downloadSelected() {
  const ids = Array.from(selectedIds.value)
  if (!ids.length) return
  const filesToDownload = ids.map(id => filesById.value.get(id)).filter((f): f is FileMeta => !!f)
  if (!filesToDownload.length) return

  // 如果只有一个文件，直接下载
  if (filesToDownload.length === 1) {
    await download(filesToDownload[0])
    return
  }

  // 多个文件：打包成zip下载
  try {
    const zip = new JSZip()
    const fileBlobs = await Promise.all(
      filesToDownload.map(async (f) => {
        const blob = await api.fetchBlob(api.downloadUrl(f.id))
        return { file: f, blob }
      })
    )
    
    // 添加文件到zip
    for (const { file, blob } of fileBlobs) {
      zip.file(file.name, blob)
    }
    
    // 生成zip文件
    const zipBlob = await zip.generateAsync({ type: 'blob' })
    
    // 保存zip文件
    const zipName = `download_${new Date().toISOString().slice(0, 10)}.zip`
    const saved = await saveBlob(zipName, zipBlob)
    
    if (saved) {
      toast(`已打包下载 ${filesToDownload.length} 个文件`, 'ok')
    }
  } catch (e: any) {
    toast(e?.message || '打包下载失败', 'error')
  }
}

function removeFiles() {
  const ids = Array.from(selectedIds.value)
  if (!ids.length) return
  confirmModal.value = { type: 'files', id: 0, name: `已选 ${ids.length} 项`, ids }
}

function removeFolder(f: Folder) {
  confirmModal.value = { type: 'folder', id: f.id, name: f.name }
}

async function doConfirmDelete() {
  const m = confirmModal.value
  if (!m) return
  confirmModal.value = null
  try {
    if (m.type === 'file') {
      await api.deleteFile(m.id)
      if (previewFile.value?.id === m.id) previewFile.value = null
      if (versionPanelFile.value?.id === m.id) versionPanelFile.value = null
    } else if (m.type === 'files') {
      const ids = m.ids || []
      for (const id of ids) await api.deleteFile(id)
      if (previewFile.value && ids.includes(previewFile.value.id)) previewFile.value = null
      if (versionPanelFile.value && ids.includes(versionPanelFile.value.id)) versionPanelFile.value = null
      clearSelection()
    } else {
      await api.deleteFolder(m.id)
      if (selectedFolder.value === m.id) selectedFolder.value = null
    }
    await load()
    toast('已删除', 'ok')
  } catch (e: any) {
    toast(e?.message || '删除失败', 'error')
  }
}

// ---------- drag & drop upload ----------
// 根目录不允许上传，必须选中具体文件夹
function canUploadTo(folderId: number | null): boolean {
  return folderId != null
}

function openUpload() {
  if (!canUploadTo(selectedFolder.value)) {
    toast('请先在文件树中选择目标文件夹', 'error')
    return
  }
  uploadOpen.value = true
}

function onDropToFolder(folderId: number | null, files: File[]) {
  if (!files.length) return
  if (!canUploadTo(folderId)) {
    toast('根目录不允许上传，请选择目标文件夹', 'error')
    return
  }
  selectedFolder.value = folderId
  droppedFiles.value = files
  uploadOpen.value = true
}

function onWindowDragOver(e: DragEvent) {
  // 应用内有内部 HTML5 拖拽（文件/文件夹移动），需区分处理
  const dt = e.dataTransfer
  if (dt) {
    // 检查是否是内部拖拽
    const hasInternalData = dt.types.includes('application/x-folder-id') || dt.types.includes('application/x-file-ids')
    if (hasInternalData) {
      // 内部拖拽：不阻止默认行为，让树节点的 drop 处理
      return
    }
    // 外部文件拖入：阻止默认行为，显示遮罩
    e.preventDefault()
    // 拖到左侧文件夹树时隐藏全屏遮罩，让节点自身的虚线高亮可见
    const target = e.target as HTMLElement | null
    const overTree = !!(target && target.closest('.tree'))
    dragOver.value = !overTree
    document.body.classList.toggle('file-dragging', !overTree)
  }
}

function onWindowDragLeave(e: DragEvent) {
  if (!e.relatedTarget) {
    dragOver.value = false
    document.body.classList.remove('file-dragging')
  }
}

async function onWindowDrop(e: DragEvent) {
  dragOver.value = false
  document.body.classList.remove('file-dragging')
  const dt = e.dataTransfer
  if (!dt) return
  // 检查是否是内部拖拽（文件夹或文件移动）
  const hasInternalData = dt.types.includes('application/x-folder-id') || dt.types.includes('application/x-file-ids')
  if (hasInternalData) {
    // 内部拖拽：由树节点的 drop.stop 处理，此处不干预
    return
  }
  // 外部文件拖入
  e.preventDefault()
  const files = await getFilesFromDropEvent(e)
  if (!files.length) return
  // 左侧树的节点有自己的 drop 处理（@drop.stop），此处放行，不重复打开对话框
  const target = e.target as HTMLElement | null
  if (target && target.closest('.tree')) return
  if (!canUploadTo(selectedFolder.value)) {
    toast('请先在文件树中选择目标文件夹', 'error')
    return
  }
  droppedFiles.value = files
  uploadOpen.value = true
}

function closeUpload() {
  uploadOpen.value = false
  droppedFiles.value = []
}

onMounted(() => {
  // 记忆的面板宽度可能超出当前窗口（换屏幕/缩窗后），先钳制到合理范围
  rightPanelWidth.value = clampPanelW(rightPanelWidth.value)
  load()
  window.addEventListener('keydown', onKeydown)
  window.addEventListener('resize', onWinResize)
  window.addEventListener('dragover', onWindowDragOver)
  window.addEventListener('dragleave', onWindowDragLeave)
  // capture：树节点 @drop.stop 会阻止冒泡，但捕获阶段窗口先执行，确保遮罩复位
  window.addEventListener('drop', onWindowDrop, true)
  window.addEventListener('dragend', () => {
    dragOver.value = false
    document.body.classList.remove('file-dragging')
  })
  // 开始轮询同步状态（每5秒检查一次）
  pollSyncStatus()
  syncPollTimer = setInterval(pollSyncStatus, 5000)
})

function onWinResize() {
  rightPanelWidth.value = clampPanelW(rightPanelWidth.value)
}

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown)
  window.removeEventListener('resize', onWinResize)
  window.removeEventListener('dragover', onWindowDragOver)
  window.removeEventListener('dragleave', onWindowDragLeave)
  window.removeEventListener('drop', onWindowDrop, true)
  // 清理同步状态轮询
  if (syncPollTimer) {
    clearInterval(syncPollTimer)
    syncPollTimer = null
  }
})

// 回到主页（清除选中的文件夹）
function goHome() {
  selectFolder(null)
}

defineExpose({ goHome })
</script>

<template>
  <div class="browser">
    <aside class="sidebar" :style="{ width: sidebarW + 'px' }">
      <FolderTree
        ref="treeRef"
        :folders="folders"
        :files="files"
        :selected="selectedFolder"
          :previewing-id="activePreviewingId"
        :root-name="rootName"
        @select="selectFolder"
        @new-folder="openNewFolder"
        @rename="(f) => openRename('folder', f.id, f.name)"
        @rename-root="openRename('root', 0, rootName)"
        @delete="removeFolder"
        @drop-files="onDropToFolder"
        @move-folder="onMoveFolder"
        @move-files="onMoveFiles"
        @open-folder="openFolderFromTree"
        @select-file="openPreview"
      />
    </aside>

    <div class="sidebar-resizer" title="拖动调整文件树宽度" @mousedown.prevent="onSidebarResizeStart"></div>

    <main class="main">
      <div
        class="main-head"
        :class="{ 'head-tight': headTight, 'head-very-tight': headVeryTight }"
      >
        <div class="crumb">
          <span class="cur-folder">{{ searchOpen ? '搜索结果' : currentFolderName }}</span>
          <span v-if="searchOpen" class="count">（{{ searchResults.length }}）</span>
          <span v-else-if="visibleFiles.length > 0" class="count">（{{ visibleFiles.length }}）</span>
        </div>
        <div class="head-ops">
          <div class="search-box">
            <input
              v-model="searchQ"
              type="text"
              placeholder="搜索文件名…"
              @keyup.enter="doSearch"
            />
            <button v-if="searchOpen" class="search-clear" @click="clearSearch">✕</button>
            <button v-else class="search-btn" @click="doSearch">搜索</button>
          </div>
          <button class="btn-refresh" title="刷新文件列表" @click="load()">刷新</button>
          <span v-if="syncStatus.is_syncing" class="sync-status syncing" title="正在同步文件系统变更">
            <span class="sync-icon"></span>同步中
          </span>
          <span v-else-if="syncStatus.last_sync_result" class="sync-status synced" :title="syncStatus.last_sync_result">
            已同步
          </span>
          <button
            class="primary"
            :disabled="!canUploadTo(selectedFolder)"
            :title="canUploadTo(selectedFolder) ? '' : '请先在文件树中选择目标文件夹'"
            @click="openUpload"
          >上传</button>
          <button class="btn-new-folder" @click="openNewFolder(selectedFolder)">新建文件夹</button>
        </div>
      </div>

      <div v-if="selectedIds.size > 0" class="batch-bar">
        <span class="batch-count">已选 {{ selectedIds.size }} 项</span>
        <button class="primary" @click="downloadSelected">下载</button>
        <button class="primary" @click="openMoveBatch">移动</button>
        <button class="danger-solid" @click="removeFiles">删除</button>
        <button @click="clearSelection">取消选择</button>
      </div>

      <!-- 属性面板（仅一级目录及以后显示） -->
      <div v-if="selectedFolderForProps || selectedFileForProps" class="property-panel-container">
        <PropertyPanel
          :folder="selectedFolderForProps"
          :file="selectedFileForProps"
          :active-tab="activePropertyTab"
          :show-inline-preview="!!selectedFileForProps"
          @update:active-tab="activePropertyTab = $event"
          @update:folder="selectedFolderForProps = $event"
          @update:file="selectedFileForProps = $event"
          @toast="toast"
          @select-folder="selectFolder"
          @preview="openPreview"
          @download="download"
          @delete="removeFile"
          @rename="(f) => openRename('file', f.id, f.name)"
          @upload="openUpload"
        />
      </div>

      <!-- 根目录显示文件列表 -->
      <div v-else class="content" @click="onContentClick">
        <FileList
          :files="visibleFiles"
          :folders="folders"
          :child-folders="childFolders"
        :previewing-id="activePreviewingId"
          :selected-ids="selectedIds"
          :all-selected="allSelected"
          :some-selected="someSelected"
          :is-root="isRoot"
          :stats="stats"
          :recent-files="recentFiles"
          @preview="openPreview"
          @download="download"
          @versions="openVersions"
          @rename="(f) => openRename('file', f.id, f.name)"
          @delete="removeFile"
          @move="openMove"
          @toggle-select="toggleSelect"
          @select-all="toggleSelectAll"
          @open-folder="openFileFolder"
          @select-folder="selectFolder"
          @upload="openUpload"
          @new-folder="openNewFolder(selectedFolder)"
        />
      </div>
    </main>

    <div v-if="previewFile || versionPanelFile" class="resizer" @mousedown.prevent="onResizeStart"></div>

    <aside v-if="versionPanelFile" class="right-panel" :style="{ width: rightPanelWidth + 'px' }">
      <VersionPanel :file="versionPanelFile" @close="versionPanelFile = null" />
    </aside>
    <aside
      v-else-if="previewFile"
      class="right-panel"
      :class="{ fullscreen: previewFullscreen }"
      :style="{ width: rightPanelWidth + 'px' }"
    >
      <div class="preview-head">
        <span class="preview-name" :title="previewFile.name">{{ previewFile.name }}</span>
        <span class="dim">v{{ previewVersion ?? previewFile.current_version }}</span>
        <span class="preview-tools">
          <button class="tool-btn" title="适应窗口" @click="fitActiveViewer">适应</button>
          <button class="tool-btn" :title="previewFullscreen ? '退出全屏' : '全屏预览'" @click="toggleFullscreen">⛶</button>
        </span>
        <button class="close" title="关闭预览" @click="previewFile = null">✕</button>
      </div>
      <div class="preview-body">
        <PreviewPane :file="previewFile" :version="previewVersion" />
      </div>
    </aside>

    <UploadDialog
      v-if="uploadOpen"
      mode="upload"
      :folder-name="currentFolderName"
      :folder-id="selectedFolder"
      :initial-files="droppedFiles"
      :existing-names="currentDirNames"
      @done="closeUpload(); load(); toast('上传完成', 'ok')"
      @close="closeUpload"
    />

    <div v-if="newFolderOpen" class="modal-mask">
      <div class="modal">
        <h3>新建文件夹</h3>
        <input
          ref="newFolderInput"
          v-model="newFolderName"
          type="text"
          placeholder="文件夹名称"
          style="width: 100%"
          @keyup.enter="createFolder"
        />
        <div class="actions">
          <button @click="newFolderOpen = false">取消</button>
          <button class="primary" @click="createFolder">创建</button>
        </div>
      </div>
    </div>

    <div v-if="renameModal" class="modal-mask">
      <div class="modal">
        <h3>重命名</h3>
        <input
          ref="renameInput"
          v-model="renameName"
          type="text"
          style="width: 100%"
          @keyup.enter="doRename"
        />
        <div class="actions">
          <button @click="renameModal = null">取消</button>
          <button class="primary" @click="doRename">确定</button>
        </div>
      </div>
    </div>

    <div v-if="moveModal" class="modal-mask">
      <div class="modal">
        <h3>
          <template v-if="moveModal.files.length > 1">移动 {{ moveModal.files.length }} 个文件</template>
          <template v-else>移动「{{ moveModal.files[0].name }}」</template>
        </h3>
        <div class="move-list">
          <label class="move-item">
            <input v-model="moveTarget" type="radio" value="root" />
            {{ rootName }}（根目录）
          </label>
          <label v-for="r in moveFolderRows" :key="r.folder.id" class="move-item" :style="{ paddingLeft: 12 + r.depth * 18 + 'px' }">
            <span class="move-ico" :class="r.depth === 0 ? '' : 'child'">📁</span>
            <input v-model="moveTarget" type="radio" :value="r.folder.id" />
            {{ r.folder.name }}
          </label>
        </div>
        <div class="actions">
          <button @click="moveModal = null">取消</button>
          <button class="primary" @click="doMove">移动</button>
        </div>
      </div>
    </div>

    <!-- 删除确认（应用内主题化模态框，替代原生 confirm；危险操作样式） -->
    <div v-if="confirmModal" class="modal-mask">
      <div class="modal confirm-modal">
        <div class="confirm-head">
          <svg
            class="confirm-ico"
            viewBox="0 0 16 16"
            width="18"
            height="18"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path d="M8 1.5 L14.5 13.5 H1.5 Z" />
            <path d="M8 6.2 V9.6" />
            <path d="M8 11.4 H8.01" />
          </svg>
          <h3>删除{{ confirmModal.type === 'folder' ? '文件夹' : '文件' }}</h3>
        </div>
        <p class="confirm-text">
          <template v-if="confirmModal.type === 'files'">
            确定删除已选 <b>{{ confirmModal.ids?.length }}</b> 个文件？其所有版本都将被删除。
          </template>
          <template v-else>
            确定删除「<b>{{ confirmModal.name }}</b>」？
            <template v-if="confirmModal.type === 'folder'">其中的所有文件和子文件夹都会被删除。</template>
            <template v-else>该文件所有版本都将被删除。</template>
          </template>
        </p>
        <p class="confirm-note">此操作不可撤销。</p>
        <div class="actions">
          <button @click="confirmModal = null">取消</button>
          <button class="danger-solid" @click="doConfirmDelete">删除</button>
        </div>
      </div>
    </div>

    <!-- 整窗拖放上传遮罩 -->
    <div v-if="dragOver" class="drop-hint">
      <div class="drop-hint-box">
        <span class="drop-hint-ico">⇪</span>
        <span>松开以上传至「{{ currentFolderName }}」</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.browser {
  display: flex;
  height: 100%;
  overflow: hidden;
}
.sidebar {
  width: var(--sidebar-w);
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  background: var(--bg-panel);
  flex-shrink: 0;
}
.side-title {
  font-weight: 600;
}
.main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  position: relative;
  overflow: hidden;
}
.main-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 8px 12px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--border);
}
.crumb {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}
.cur-folder {
  font-weight: 600;
  max-width: 300px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.count {
  color: var(--text-dim);
}
.head-ops {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  flex-wrap: wrap;
}
.search-box {
  display: flex;
  gap: 4px;
  flex: 1 1 auto;
  min-width: 0;
  align-items: center;
}
.search-box input {
  width: auto;
  flex: 1 1 220px;
  min-width: 60px;
}
.search-box .search-btn,
.search-box .search-clear {
  flex-shrink: 0;
}
.btn-refresh {
  background: transparent;
  border: 1px solid var(--border);
  color: var(--text-dim);
  padding: 3px 10px;
  font-size: var(--font-sm);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background var(--transition-fast), color var(--transition-fast);
}
.btn-refresh:hover {
  background: var(--bg-hover);
  color: var(--text);
}
.sync-status {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 3px 8px;
  font-size: var(--font-sm);
  border-radius: var(--radius-sm);
  white-space: nowrap;
}
.sync-status.syncing {
  color: var(--accent);
  background: var(--accent-soft);
}
.sync-status.synced {
  color: var(--text-dim);
  background: transparent;
}
.sync-icon {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--accent);
  animation: pulse 1.5s ease-in-out infinite;
}
@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.4; }
}
.content {
  flex: 1;
  overflow: hidden;
  min-height: 0;
}
.batch-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 14px;
  border-bottom: 1px solid var(--border);
  background: var(--accent-soft);
}
.batch-count {
  font-weight: 600;
  color: var(--accent);
  margin-right: 4px;
}
.batch-bar button {
  padding: 3px 12px;
  font-size: var(--font-sm);
}
.property-panel-container {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  border-bottom: 1px solid var(--border);
}
.drop-hint {
  position: fixed;
  inset: 0;
  z-index: 200;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--accent-soft-faint);
  backdrop-filter: blur(1px);
  pointer-events: none;
}
.drop-hint-box {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 28px 40px;
  border: 2px dashed var(--accent);
  border-radius: 14px;
  background: var(--bg-panel);
  color: var(--accent);
  font-size: 16px;
  font-weight: 600;
  box-shadow: var(--shadow);
}
.drop-hint-ico {
  font-size: 26px;
  line-height: 1;
}
.right-panel {
  border-left: 1px solid var(--border);
  background: var(--bg-panel);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  min-width: 0;
}
.right-panel.fullscreen {
  position: fixed;
  inset: 0;
  width: 100% !important;
  z-index: 150;
}
.resizer {
  width: 6px;
  flex-shrink: 0;
  cursor: col-resize;
  background: transparent;
  transition: background 0.15s;
}
.resizer:hover,
.resizer:active {
  background: var(--accent-soft);
}
.sidebar-resizer {
  width: 6px;
  flex-shrink: 0;
  cursor: col-resize;
  background: transparent;
  transition: background 0.15s;
  position: relative;
}
.sidebar-resizer::after {
  content: '';
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  width: 2px;
  height: 32px;
  border-radius: 1px;
  background: var(--text-faint);
  opacity: 0;
  transition: opacity 0.15s;
}
.sidebar-resizer:hover::after {
  opacity: 1;
}
.sidebar-resizer:hover,
.sidebar-resizer:active {
  background: var(--accent-soft);
}
.preview-tools {
  display: flex;
  align-items: center;
  gap: 2px;
  margin-left: auto;
}
.tool-btn {
  padding: 2px 6px;
  font-size: var(--font-sm);
  background: transparent;
  border: none;
  color: var(--text-dim);
  border-radius: var(--radius-sm);
}
.tool-btn:hover:not(:disabled) {
  background: var(--bg-hover);
  color: var(--text);
}
.preview-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--border);
}
.preview-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 600;
}
.close {
  border: none;
  background: transparent;
  color: var(--text-dim);
}
.close:hover {
  color: var(--text);
}
.preview-body {
  flex: 1;
  overflow: hidden;
  min-height: 0;
}
.move-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 300px;
  overflow: auto;
}
.move-item {
  display: flex;
  align-items: center;
  gap: 6px;
}
.move-ico {
  font-size: 13px;
  flex-shrink: 0;
}
.move-ico.child {
  opacity: 0.6;
}
.confirm-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 10px;
}
.confirm-ico {
  color: var(--danger);
  flex-shrink: 0;
}
.confirm-modal h3 {
  margin: 0;
  color: var(--danger);
}
.confirm-text {
  margin: 0 0 6px;
  line-height: 1.5;
}
.confirm-text b {
  word-break: break-all;
}
.confirm-note {
  margin: 0;
  font-size: var(--font-sm);
  color: var(--text-dim);
}
.danger-solid {
  background: var(--danger);
  border-color: var(--danger);
  color: var(--text-on-danger);
}
.danger-solid:hover:not(:disabled) {
  background: var(--danger);
  border-color: var(--danger);
  filter: brightness(0.92);
}
@media (max-width: 1120px) {
  .search-box input {
    width: 150px;
  }
  .btn-new-folder {
    display: none;
  }
}
@media (max-width: 1000px) {
  .search-box .search-btn {
    display: none;
  }
  .search-box input {
    width: 120px;
  }
  .cur-folder {
    max-width: 80px;
  }
  .head-ops {
    gap: 6px;
  }
}
/* 主区被预览面板挤压变窄时（JS 计算 mainAreaWidth），收起次要按钮，避免头部溢出被面板盖住 */
.main-head.head-tight .btn-new-folder {
  display: none;
}
.main-head.head-very-tight .search-box .search-btn {
  display: none;
}
</style>
