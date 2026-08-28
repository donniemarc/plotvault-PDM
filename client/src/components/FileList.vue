<script setup lang="ts">
import type { FileMeta, Folder } from '../types'
import { fileBadge, formatBytes, formatDate } from '../utils'
import { ref, onMounted, onBeforeUnmount } from 'vue'
import DashboardChart from './DashboardChart.vue'

const props = defineProps<{
  files: FileMeta[]
  folders?: Folder[]
  childFolders?: Folder[]
  previewingId?: number | null
  selectedIds?: Set<number>
  allSelected?: boolean
  someSelected?: boolean
  isRoot?: boolean
  stats?: { fileCount: number; folderCount: number; totalSize: number }
  recentFiles?: FileMeta[]
}>()
const emit = defineEmits<{
  preview: [file: FileMeta]
  download: [file: FileMeta]
  versions: [file: FileMeta]
  rename: [file: FileMeta]
  delete: [file: FileMeta]
  move: [file: FileMeta]
  upload: []
  'new-folder': []
  'toggle-select': [id: number]
  'select-all': [checked: boolean]
  'open-folder': [file: FileMeta]
  'select-folder': [folderId: number]
}>()

const draggingId = ref<number | null>(null)

// 右键菜单
const contextMenu = ref<{ x: number; y: number; file: FileMeta } | null>(null)

function onContextMenu(e: MouseEvent, file: FileMeta) {
  e.preventDefault()
  // 计算菜单位置，避免底部截断
  const menuHeight = 300 // 预估菜单高度（FileList菜单项更多）
  const viewportHeight = window.innerHeight
  let y = e.clientY
  if (y + menuHeight > viewportHeight) {
    y = Math.max(0, viewportHeight - menuHeight - 10)
  }
  let x = e.clientX
  const menuWidth = 200
  if (x + menuWidth > window.innerWidth) {
    x = window.innerWidth - menuWidth - 10
  }
  contextMenu.value = { x, y, file }
}

function closeContextMenu() {
  contextMenu.value = null
}

function ctxAction(action: string) {
  if (!contextMenu.value) return
  const file = contextMenu.value.file
  closeContextMenu()
  if (action === 'open-folder') emit('open-folder', file)
  else if (action === 'preview') emit('preview', file)
  else if (action === 'download') emit('download', file)
  else if (action === 'versions') emit('versions', file)
  else if (action === 'rename') emit('rename', file)
  else if (action === 'move') emit('move', file)
  else if (action === 'delete') emit('delete', file)
}

function onGlobalClick() {
  closeContextMenu()
}

onMounted(() => document.addEventListener('click', onGlobalClick))
onBeforeUnmount(() => document.removeEventListener('click', onGlobalClick))

function onDragStartFile(e: DragEvent, file: FileMeta) {
  const dt = e.dataTransfer
  if (dt) {
    // 如果文件在选中列表中，拖拽所有选中文件；否则只拖拽当前文件
    const isSelected = props.selectedIds?.has(file.id)
    const ids = isSelected
      ? Array.from(props.selectedIds || [])
      : [file.id]
    dt.setData('application/x-file-ids', JSON.stringify(ids))
    dt.effectAllowed = 'move'
  }
  draggingId.value = file.id
}

function onDragEndFile() {
  draggingId.value = null
}
</script>

<template>
  <div class="list-wrap">
    <!-- 根目录空状态：品牌展示 + 数据概览 -->
    <div v-if="files.length === 0 && isRoot" class="welcome" @contextmenu.prevent @drop.prevent @dragover.prevent>
      <div class="welcome-brand">
        <div class="welcome-logo">📁</div>
        <h1 class="welcome-title">PlotVault PDM</h1>
        <p class="welcome-desc">轻量级个人 NAS 图纸文档管理系统</p>
      </div>
      <div v-if="stats" class="welcome-stats">
        <div class="stat-item">
          <span class="stat-value">{{ stats.fileCount }}</span>
          <span class="stat-label">文件</span>
        </div>
        <div class="stat-item">
          <span class="stat-value">{{ stats.folderCount }}</span>
          <span class="stat-label">文件夹</span>
        </div>
        <div class="stat-item">
          <span class="stat-value">{{ formatBytes(stats.totalSize) }}</span>
          <span class="stat-label">总大小</span>
        </div>
      </div>
      <DashboardChart :files="files" :folders="folders || []" />
      <div v-if="recentFiles && recentFiles.length > 0" class="welcome-recent">
        <h3>最近修改</h3>
        <div class="recent-list">
          <div v-for="f in recentFiles" :key="f.id" class="recent-item" @click="emit('preview', f)">
            <span class="badge" :class="fileBadge(f.ext).c">{{ fileBadge(f.ext).t }}</span>
            <span class="recent-name">{{ f.name }}</span>
            <span class="recent-date dim">{{ formatDate(f.updated_at) }}</span>
          </div>
        </div>
      </div>
      <div class="welcome-tip">点击右上角「上传」添加文件，或从左侧文件树开始浏览</div>
    </div>
    <!-- 普通目录空状态 -->
    <div class="empty" v-else-if="files.length === 0" @contextmenu.prevent @drop.prevent @dragover.prevent>
      <!-- 有子文件夹：卡片展示，点击进入 -->
      <template v-if="childFolders && childFolders.length > 0">
        <div class="empty-with-folders">
          <div class="empty-icon">📁</div>
          <div class="empty-text">此文件夹没有文件，包含 {{ childFolders.length }} 个子文件夹</div>
          <div class="empty-hint">点击卡片进入子文件夹</div>
        </div>
        <div class="subfolder-grid">
          <div
            v-for="cf in childFolders"
            :key="cf.id"
            class="subfolder-card"
            @click="emit('select-folder', cf.id)"
            :title="cf.name"
          >
            <span class="subfolder-ico">📁</span>
            <span class="subfolder-name">{{ cf.name }}</span>
          </div>
        </div>
      </template>
      <!-- 完全为空：提示 + 快捷按钮 -->
      <template v-else>
        <div class="empty-with-folders">
          <div class="empty-icon">📂</div>
          <div class="empty-text">此目录为空</div>
        </div>
        <div class="empty-actions">
          <button class="primary" @click="emit('upload')">上传文件</button>
          <button @click="emit('new-folder')">新建文件夹</button>
        </div>
      </template>
    </div>
    <table v-else class="file-table">
      <thead>
        <tr>
          <th class="chk-col">
            <input
              type="checkbox"
              :checked="allSelected"
              :indeterminate="someSelected && !allSelected"
              @click.stop
              @change="emit('select-all', ($event.target as HTMLInputElement).checked)"
            />
          </th>
          <th style="width: 64px">类型</th>
          <th>名称</th>
          <th style="width: 72px">版本</th>
          <th style="width: 88px">大小</th>
          <th style="width: 150px">更新时间</th>
          <th style="width: 216px">操作</th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="f in files"
          :key="f.id"
          :class="{ previewing: f.id === previewingId, dragging: draggingId === f.id }"
          draggable="true"
          @click="emit('preview', f)"
          @contextmenu="onContextMenu($event, f)"
          @dragstart="onDragStartFile($event, f)"
          @dragend="onDragEndFile"
        >
          <td class="chk-col" @click.stop>
            <input
              type="checkbox"
              :checked="selectedIds?.has(f.id) ?? false"
              @click.stop
              @change="emit('toggle-select', f.id)"
            />
          </td>
          <td><span class="badge" :class="fileBadge(f.ext).c">{{ fileBadge(f.ext).t }}</span></td>
          <td class="fname" :title="f.name">{{ f.name }}</td>
          <td class="center">v{{ f.current_version }}</td>
          <td class="dim">{{ formatBytes(f.size) }}</td>
          <td class="dim">{{ formatDate(f.updated_at) }}</td>
          <td>
            <div class="ops">
              <button title="预览" @click.stop="emit('preview', f)">预览</button>
              <button title="下载" @click.stop="emit('download', f)">下载</button>
              <button title="版本历史" @click.stop="emit('versions', f)">版本</button>
              <button class="op-rename" title="重命名" @click.stop="emit('rename', f)">✎</button>
              <button class="op-move" title="移动" @click.stop="emit('move', f)">⇄</button>
              <button class="op-del" title="删除" @click.stop="emit('delete', f)">🗑</button>
            </div>
          </td>
        </tr>
      </tbody>
    </table>

    <!-- 右键菜单 -->
    <Teleport to="body">
      <div
        v-if="contextMenu"
        class="context-menu"
        :style="{ left: contextMenu.x + 'px', top: contextMenu.y + 'px' }"
        @click.stop
      >
        <div class="ctx-item" @click="ctxAction('open-folder')">
          <span class="ctx-ico">📂</span> 打开文件所在文件夹
        </div>
        <div class="ctx-sep" />
        <div class="ctx-item" @click="ctxAction('preview')">
          <span class="ctx-ico">👁</span> 预览
        </div>
        <div class="ctx-item" @click="ctxAction('download')">
          <span class="ctx-ico">⬇</span> 下载
        </div>
        <div class="ctx-item" @click="ctxAction('versions')">
          <span class="ctx-ico">📋</span> 版本历史
        </div>
        <div class="ctx-sep" />
        <div class="ctx-item" @click="ctxAction('rename')">
          <span class="ctx-ico">✎</span> 重命名
        </div>
        <div class="ctx-item" @click="ctxAction('move')">
          <span class="ctx-ico">⇄</span> 移动
        </div>
        <div class="ctx-sep" />
        <div class="ctx-item ctx-danger" @click="ctxAction('delete')">
          <span class="ctx-ico">🗑</span> 删除
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.list-wrap {
  height: 100%;
  overflow: auto;
}
.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  padding: 40px;
  gap: 24px;
  color: var(--text-dim);
  overflow: auto;
}
.empty-with-folders {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  text-align: center;
}
.empty-icon {
  font-size: 48px;
}
.empty-text {
  font-size: var(--font-base);
  color: var(--text-dim);
}
.empty-hint {
  font-size: var(--font-sm);
  color: var(--text-faint);
}
.empty-actions {
  display: flex;
  gap: 10px;
}
/* 子文件夹卡片网格 */
.subfolder-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: 10px;
  width: 100%;
  max-width: 640px;
}
.subfolder-card {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 14px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background var(--transition-fast), border-color var(--transition-fast);
}
.subfolder-card:hover {
  background: var(--bg-hover);
  border-color: var(--accent);
}
.subfolder-ico {
  font-size: 18px;
  flex-shrink: 0;
}
.subfolder-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text);
}

/* 欢迎页面 */
.welcome {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  padding: 40px;
  gap: 32px;
}
.welcome-brand {
  text-align: center;
}
.welcome-logo {
  font-size: 64px;
  margin-bottom: 16px;
}
.welcome-title {
  font-size: 28px;
  font-weight: 700;
  margin: 0 0 8px;
  color: var(--text);
}
.welcome-desc {
  font-size: var(--font-base);
  color: var(--text-dim);
  margin: 0;
}
.welcome-stats {
  display: flex;
  gap: 40px;
}
.stat-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
}
.stat-value {
  font-size: 24px;
  font-weight: 700;
  color: var(--accent);
}
.stat-label {
  font-size: var(--font-sm);
  color: var(--text-dim);
}
.welcome-recent {
  width: 100%;
  max-width: 500px;
}
.welcome-recent h3 {
  font-size: var(--font-base);
  margin: 0 0 12px;
  color: var(--text-dim);
  text-align: center;
}
.recent-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.recent-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background var(--transition-fast);
}
.recent-item:hover {
  background: var(--bg-hover);
}
.recent-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.recent-date {
  font-size: var(--font-sm);
  flex-shrink: 0;
}
.welcome-tip {
  font-size: var(--font-sm);
  color: var(--text-faint);
  text-align: center;
}

.file-table {
  width: 100%;
  border-collapse: collapse;
  table-layout: fixed;
}
.file-table th {
  position: sticky;
  top: 0;
  background: var(--bg-panel);
  text-align: left;
  color: var(--text-dim);
  font-weight: 500;
  padding: 8px 10px;
  border-bottom: 1px solid var(--border);
  z-index: 1;
}
.file-table td {
  padding: 6px 10px;
  border-bottom: 1px solid var(--border-faint);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.file-table tr {
  cursor: pointer;
}
.file-table tbody tr:hover {
  background: var(--bg-hover);
}
.file-table tbody tr.previewing {
  background: var(--bg-active);
}
.file-table tbody tr.dragging {
  opacity: 0.4;
}
.file-table tbody tr.previewing .fname {
  color: var(--accent);
}
.fname {
  min-width: 140px;
  max-width: none;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.center {
  text-align: center;
}
.dim {
  color: var(--text-dim);
}
.chk-col {
  width: 32px;
  text-align: center;
  padding: 6px 4px !important;
}
.chk-col input[type='checkbox'] {
  cursor: pointer;
  accent-color: var(--accent);
}
.ops {
  display: flex;
  gap: 4px;
}
.ops button {
  padding: 2px 8px;
  font-size: var(--font-sm);
}
.op-rename,
.op-move {
  background: transparent;
  border: none;
  color: var(--text-dim);
  padding: 2px 5px;
}
.op-rename:hover,
.op-move:hover {
  background: var(--bg-hover);
  color: var(--text);
}
.op-del {
  background: transparent;
  border: none;
  color: var(--danger);
  padding: 2px 5px;
}
.op-del:hover {
  background: var(--danger-soft);
  color: var(--danger);
}

/* 右键菜单 */
.context-menu {
  position: fixed;
  z-index: 9999;
  min-width: 180px;
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
.ctx-danger {
  color: var(--danger);
}
.ctx-danger:hover {
  background: var(--danger-soft);
}
</style>