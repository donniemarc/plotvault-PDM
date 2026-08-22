<script setup lang="ts">
import { computed, ref } from 'vue'
import type { Folder } from '../types'

const props = defineProps<{
  folders: Folder[]
  selected: number | null
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
}>()

// 标准逐层展开：打开默认展开根节点显示一级目录，子级点击再展开
const rootExpanded = ref(true)
const expanded = ref<Set<number>>(new Set())
const dragOverId = ref<number | 'root' | null>(null)
const dragSource = ref<{ type: 'folder' | 'files'; id: number | number[] } | null>(null)

// guides 已移除：用缩进展示层级
const rows = computed(() => {
  const children = new Map<number, Folder[]>()
  for (const f of props.folders) {
    const key = f.parent_id ?? 0
    if (!children.has(key)) children.set(key, [])
    children.get(key)!.push(f)
  }
  const out: { folder: Folder; depth: number; hasChildren: boolean }[] = []
  const walk = (pid: number, depth: number) => {
    const list = children.get(pid) || []
    list.forEach((f) => {
      out.push({ folder: f, depth, hasChildren: (children.get(f.id) || []).length > 0 })
      if (expanded.value.has(f.id)) walk(f.id, depth + 1)
    })
  }
  // 根节点折叠时不显示子级；顶级文件夹从 depth=1 开始，与根节点缩进错开
  if (rootExpanded.value) walk(0, 1)
  return out
})

function toggle(folder: Folder) {
  if (!rows.value.find((r) => r.folder.id === folder.id)?.hasChildren) return
  const s = new Set(expanded.value)
  if (s.has(folder.id)) s.delete(folder.id)
  else s.add(folder.id)
  expanded.value = s
}

function isExpanded(folder: Folder) {
  return expanded.value.has(folder.id)
}

function onDropNode(e: DragEvent, folderId: number | null) {
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
  // 外部文件拖入上传
  const files = dt?.files
  if (!files || !files.length) return
  emit('drop-files', folderId, Array.from(files))
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
  <div class="tree">
    <div
      class="node"
      :class="{ selected: selected === null }"
      style="padding-left: 8px"
      tabindex="0"
      @click="emit('select', null)"
      @keydown.enter.prevent="emit('select', null)"
    >
      <span
        class="chevron"
        :class="{ 'no-child': !props.folders.length }"
        @click.stop="rootExpanded = !rootExpanded"
        >{{ props.folders.length ? (rootExpanded ? '▾' : '▸') : '' }}</span
      >
      <span class="folder-ico" @click.stop="rootExpanded = !rootExpanded">📁</span>
      <span class="name" @click="emit('select', null)" :title="rootName">{{ rootName }}</span>
      <span class="acts">
        <button class="mini" title="新建文件夹" @click.stop="emit('new-folder', null)">＋</button>
        <button class="mini" title="重命名根目录" @click.stop="emit('renameRoot')">✎</button>
      </span>
    </div>
    <div
      v-for="r in rows"
      :key="r.folder.id"
      class="node"
      :class="{
        selected: selected === r.folder.id,
        'drag-over': dragOverId === r.folder.id,
        'dragging': dragSource?.type === 'folder' && dragSource?.id === r.folder.id
      }"
      :style="{ paddingLeft: 8 + r.depth * 16 + 'px' }"
      tabindex="0"
      draggable="true"
      @click="emit('select', r.folder.id)"
      @dblclick.stop="toggle(r.folder)"
      @keydown.enter.prevent="emit('select', r.folder.id)"
      @dragover.prevent="dragOverId = r.folder.id"
      @dragleave="dragOverId = null"
      @drop.stop="onDropNode($event, r.folder.id)"
      @dragstart="onDragStartFolder($event, r.folder.id)"
      @dragend="onDragEnd"
    >
      <span class="chevron" @click.stop="toggle(r.folder)" :class="{ 'no-child': !r.hasChildren }">
        {{ r.hasChildren ? (isExpanded(r.folder) ? '▾' : '▸') : '' }}
      </span>
      <span class="folder-ico" @click="toggle(r.folder)">📁</span>
      <span class="name" @click="emit('select', r.folder.id)" :title="r.folder.name">{{ r.folder.name }}</span>
      <span class="acts">
        <button class="mini" title="新建子文件夹" @click.stop="emit('new-folder', r.folder.id)">＋</button>
        <button class="mini" title="重命名" @click.stop="emit('rename', r.folder)">✎</button>
        <button class="mini" title="删除" @click.stop="emit('delete', r.folder)">🗑</button>
      </span>
    </div>
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
</style>
