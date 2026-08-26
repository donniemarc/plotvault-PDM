<script setup lang="ts">
import { computed, ref, watch, onMounted, onBeforeUnmount } from 'vue'
import type { Folder, FileMeta } from '../types'
import { getFilesFromDropEvent, fileBadge } from '../utils'

const TREE_EXPANDED_KEY = 'plotvault_pdm_tree_expanded'

function loadExpanded(): Set<number> {
  try {
    const raw = localStorage.getItem(TREE_EXPANDED_KEY)
    if (raw) {
      const arr = JSON.parse(raw)
      if (Array.isArray(arr)) return new Set(arr)
    }
  } catch { /* ignore */ }
  return new Set()
}

function saveExpanded(s: Set<number>) {
  localStorage.setItem(TREE_EXPANDED_KEY, JSON.stringify(Array.from(s)))
}

const props = defineProps<{
  folders: Folder[]
  files: FileMeta[]
  selected: number | null
  previewingId?: number | null
  rootName: string
}>()
const emit = defineEmits<{
  select: [id: number | null]
  'new-folder': [parentId: number | null]
  rename: [folder: Folder]
  renameRoot: []
  delete: [folder: Folder]
  'drop-files': [folderId: number | null, files: File[]]
  'move-folder': [folderId: number, targetParentId: number | null]
  'move-files': [fileIds: number[], targetFolderId: number | null]
  'open-folder': [folderId: number | null]
  'select-file': [file: FileMeta]
}>()

// 标准逐层展开：打开默认展开根节点显示一级目录，子级点击再展开
const rootExpanded = ref(true)
const expanded = ref<Set<number>>(loadExpanded())
const dragOverId = ref<number | 'root' | null>(null)
const dragSource = ref<{ type: 'folder' | 'files'; id: number | number[] } | null>(null)

watch(expanded, (s) => saveExpanded(s))

// 根节点是否有可展开内容（有子文件夹或根目录直属文件）
const rootHasChildren = computed(() => {
  if (props.folders.length > 0) return true
  return props.files.some((f) => f.folder_id == null)
})

// 右键菜单
const contextMenu = ref<{ x: number; y: number; folderId: number | null; folderName: string; folder: Folder | null } | null>(null)

function onContextMenu(e: MouseEvent, folderId: number | null, folderName: string, folder: Folder | null) {
  e.preventDefault()
  contextMenu.value = { x: e.clientX, y: e.clientY, folderId, folderName, folder }
}

function closeContextMenu() {
  contextMenu.value = null
}

function ctxAction(action: string) {
  if (!contextMenu.value) return
  const { folderId, folder } = contextMenu.value
  closeContextMenu()
  if (action === 'open-folder') emit('open-folder', folderId)
  else if (action === 'new-folder') emit('new-folder', folderId)
  else if (action === 'rename-root') emit('renameRoot')
  else if (action === 'rename' && folder) emit('rename', folder)
  else if (action === 'delete' && folder) emit('delete', folder)
}

function onGlobalClick() {
  closeContextMenu()
}

onMounted(() => document.addEventListener('click', onGlobalClick))
onBeforeUnmount(() => document.removeEventListener('click', onGlobalClick))

// guides 已移除：用缩进展示层级
const rows = computed(() => {
  const children = new Map<number, Folder[]>()
  for (const f of props.folders) {
    const key = f.parent_id ?? 0
    if (!children.has(key)) children.set(key, [])
    children.get(key)!.push(f)
  }
  // 按 folder_id 分组文件
  const filesByFolder = new Map<number, FileMeta[]>()
  for (const f of props.files) {
    const key = f.folder_id ?? 0
    if (!filesByFolder.has(key)) filesByFolder.set(key, [])
    filesByFolder.get(key)!.push(f)
  }
  const out: { type: 'folder' | 'file'; folder?: Folder; file?: FileMeta; depth: number; hasChildren?: boolean }[] = []
  const walk = (pid: number, depth: number) => {
    const list = children.get(pid) || []
    list.forEach((f) => {
      // 有子文件夹或有直属文件即可展开/收起（否则只有文件的目录没有三角，无法展开看到文件）
      out.push({
        type: 'folder',
        folder: f,
        depth,
        hasChildren: (children.get(f.id) || []).length > 0 || (filesByFolder.get(f.id) || []).length > 0,
      })
      if (expanded.value.has(f.id)) {
        walk(f.id, depth + 1)
        // 展开的文件夹下显示直属文件
        const folderFiles = filesByFolder.get(f.id) || []
        for (const ff of folderFiles) {
          out.push({ type: 'file', file: ff, depth: depth + 1 })
        }
      }
    })
  }
  // 根节点折叠时不显示子级；顶级文件夹从 depth=1 开始，与根节点缩进错开
  if (rootExpanded.value) {
    // 根目录直属文件
    const rootFiles = filesByFolder.get(0) || []
    for (const ff of rootFiles) {
      out.push({ type: 'file', file: ff, depth: 1 })
    }
    walk(0, 1)
  }
  return out
})

function toggle(folder: Folder) {
  if (!rows.value.find((r) => r.type === 'folder' && r.folder?.id === folder.id)?.hasChildren) return
  const s = new Set(expanded.value)
  if (s.has(folder.id)) s.delete(folder.id)
  else s.add(folder.id)
  expanded.value = s
}

function isExpanded(folder: Folder) {
  return expanded.value.has(folder.id)
}

async function onDropNode(e: DragEvent, folderId: number | null) {
  dragOverId.value = null
  // 检查是否是内部拖拽（文件夹或文件移动）
  const dt = e.dataTransfer
  if (dt) {
    const folderDragId = dt.getData('application/x-folder-id')
    const filesDragIds = dt.getData('application/x-file-ids')
    if (folderDragId) {
      const draggedId = parseInt(folderDragId, 10)
      if (!isNaN(draggedId) && draggedId !== folderId) {
        emit('move-folder', draggedId, folderId)
      }
      return
    }
    if (filesDragIds) {
      try {
        const ids: number[] = JSON.parse(filesDragIds)
        if (ids.length > 0) {
          emit('move-files', ids, folderId)
        }
      } catch { /* ignore */ }
      return
    }
  }
  // 外部文件拖入上传（支持文件夹递归读取）
  const files = await getFilesFromDropEvent(e)
  if (!files.length) return
  emit('drop-files', folderId, files)
}

function onDragStartFolder(e: DragEvent, folderId: number) {
  const dt = e.dataTransfer
  if (dt) {
    dt.setData('application/x-folder-id', String(folderId))
    dt.effectAllowed = 'move'
  }
  dragSource.value = { type: 'folder', id: folderId }
}

function onDragEnd() {
  dragSource.value = null
  dragOverId.value = null
}

// 展开到指定文件夹（含根节点与所有祖先），使新建的文件夹直接可见
function expandTo(folderId: number) {
  rootExpanded.value = true
  const s = new Set(expanded.value)
  const parentMap = new Map<number, number>()
  for (const f of props.folders) {
    if (f.parent_id != null) parentMap.set(f.id, f.parent_id)
  }
  let cur: number | undefined = folderId
  while (cur != null) {
    s.add(cur)
    cur = parentMap.get(cur)
  }
  expanded.value = s
}

defineExpose({ expandTo })
</script>

<template>
  <div class="tree" @contextmenu.prevent>
    <div
      class="node"
      :class="{ selected: selected === null, 'drag-over': dragOverId === null && dragSource }"
      style="padding-left: 8px"
      tabindex="0"
      @click="emit('select', null)"
      @keydown.enter.prevent="emit('select', null)"
      @dragover.prevent="dragOverId = null"
      @drop.stop="onDropNode($event, null)"
      @contextmenu="onContextMenu($event, null, rootName, null)"
    >
      <span
        class="chevron"
        :class="{ 'no-child': !rootHasChildren }"
        @click.stop="rootExpanded = !rootExpanded"
        >{{ rootHasChildren ? (rootExpanded ? '▾' : '▸') : '' }}</span
      >
      <span class="folder-ico" @click.stop="rootExpanded = !rootExpanded">📁</span>
      <span class="name" @click="emit('select', null)" :title="rootName">{{ rootName }}</span>
      <span class="acts">
        <button class="mini" title="新建文件夹" @click.stop="emit('new-folder', null)">＋</button>
        <button class="mini" title="重命名根目录" @click.stop="emit('renameRoot')">✎</button>
      </span>
    </div>
    <template v-for="r in rows" :key="r.type === 'folder' ? 'f-' + r.folder!.id : 'ff-' + r.file!.id">
      <!-- 文件夹节点 -->
      <div
        v-if="r.type === 'folder'"
        class="node"
        :class="{
          selected: selected === r.folder!.id,
          'drag-over': dragOverId === r.folder!.id,
          'dragging': dragSource?.type === 'folder' && dragSource?.id === r.folder!.id
        }"
        :style="{ paddingLeft: 8 + r.depth * 16 + 'px' }"
        tabindex="0"
        draggable="true"
        @click="emit('select', r.folder!.id)"
        @dblclick.stop="toggle(r.folder!)"
        @keydown.enter.prevent="emit('select', r.folder!.id)"
        @dragover.prevent="dragOverId = r.folder!.id"
        @dragleave="dragOverId = null"
        @drop.stop="onDropNode($event, r.folder!.id)"
        @dragstart="onDragStartFolder($event, r.folder!.id)"
        @dragend="onDragEnd"
        @contextmenu="onContextMenu($event, r.folder!.id, r.folder!.name, r.folder!)"
      >
        <span class="chevron" @click.stop="toggle(r.folder!)" :class="{ 'no-child': !r.hasChildren }">
          {{ r.hasChildren ? (isExpanded(r.folder!) ? '▾' : '▸') : '' }}
        </span>
        <span class="folder-ico" @click.stop="toggle(r.folder!)">📁</span>
        <span class="name" @click="emit('select', r.folder!.id)" :title="r.folder!.name">{{ r.folder!.name }}</span>
        <span class="acts">
          <button class="mini" title="新建子文件夹" @click.stop="emit('new-folder', r.folder!.id)">＋</button>
          <button class="mini" title="重命名" @click.stop="emit('rename', r.folder!)">✎</button>
          <button class="mini" title="删除" @click.stop="emit('delete', r.folder!)">🗑</button>
        </span>
      </div>
      <!-- 文件节点 -->
      <div
        v-else
        class="node tree-file"
        :class="{ previewing: r.file!.id === previewingId }"
        :style="{ paddingLeft: 8 + r.depth * 16 + 'px' }"
        @click="emit('select-file', r.file!)"
      >
        <span class="chevron no-child" />
        <span class="file-badge">{{ fileBadge(r.file!.ext).t }}</span>
        <span class="name" :title="r.file!.name">{{ r.file!.name }}</span>
      </div>
    </template>

    <!-- 右键菜单 -->
    <Teleport to="body">
      <div
        v-if="contextMenu"
        class="context-menu"
        :style="{ left: contextMenu.x + 'px', top: contextMenu.y + 'px' }"
        @click.stop
      >
        <div class="ctx-item" @click="ctxAction('open-folder')">
          <span class="ctx-ico">📂</span> 打开文件夹
        </div>
        <div class="ctx-sep" />
        <div class="ctx-item" @click="ctxAction('new-folder')">
          <span class="ctx-ico">📁</span> 新建子文件夹
        </div>
        <template v-if="contextMenu.folder">
          <div class="ctx-sep" />
          <div class="ctx-item" @click="ctxAction('rename')">
            <span class="ctx-ico">✎</span> 重命名
          </div>
          <div class="ctx-sep" />
          <div class="ctx-item ctx-danger" @click="ctxAction('delete')">
            <span class="ctx-ico">🗑</span> 删除
          </div>
        </template>
        <template v-else>
          <div class="ctx-sep" />
          <div class="ctx-item" @click="ctxAction('rename-root')">
            <span class="ctx-ico">✎</span> 重命名根目录
          </div>
        </template>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.tree {
  padding: 6px;
  overflow-y: auto;
  flex: 1;
}
.node {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 5px 6px;
  border-radius: 6px;
  cursor: pointer;
  white-space: nowrap;
}
.node:hover {
  background: var(--bg-hover);
}
.node.selected {
  background: var(--bg-active);
  box-shadow: inset 3px 0 0 0 var(--accent);
}
.node.drag-over {
  outline: 2px dashed var(--accent);
  outline-offset: -2px;
  background: var(--accent-soft);
}
.node.dragging {
  opacity: 0.4;
}
.chevron {
  width: 14px;
  text-align: center;
  color: var(--text-dim);
  user-select: none;
  flex-shrink: 0;
}
.chevron.no-child {
  cursor: default;
}
.folder-ico {
  font-size: 14px;
  flex-shrink: 0;
}
.tree-file {
  cursor: pointer;
  opacity: 0.85;
}
.tree-file:hover {
  opacity: 1;
}
.tree-file.previewing {
  background: var(--bg-active);
  box-shadow: inset 3px 0 0 0 var(--accent);
}
.file-badge {
  font-size: 9px;
  padding: 1px 4px;
  border-radius: 3px;
  background: var(--bg-hover);
  color: var(--text-dim);
  flex-shrink: 0;
  font-weight: 500;
  letter-spacing: 0.3px;
}
.name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
}
.acts {
  display: none;
  gap: 2px;
  flex-shrink: 0;
}
.node:hover .acts,
.node:focus-within .acts {
  display: flex;
}
.mini {
  padding: 1px 5px;
  font-size: 11px;
  border: none;
  background: transparent;
  color: var(--text-dim);
}
.mini:hover {
  background: var(--bg-active);
  color: var(--text);
}

/* 右键菜单 */
.context-menu {
  position: fixed;
  z-index: 9999;
  min-width: 160px;
  background: var(--bg-panel);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  box-shadow: var(--shadow-md);
  padding: 4px 0;
}
.ctx-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 14px;
  font-size: var(--font-sm);
  cursor: pointer;
  transition: background var(--transition-fast);
}
.ctx-item:hover {
  background: var(--bg-hover);
}
.ctx-ico {
  font-size: 13px;
  width: 18px;
  text-align: center;
}
.ctx-sep {
  height: 1px;
  background: var(--border-faint);
  margin: 4px 0;
}
</style>
