<script setup lang="ts">
import type { FileMeta } from '../types'
import { fileBadge, formatBytes, formatDate } from '../utils'

defineProps<{
  files: FileMeta[]
  previewingId?: number | null
  selectedIds?: Set<number>
  allSelected?: boolean
  someSelected?: boolean
}>()
const emit = defineEmits<{
  preview: [file: FileMeta]
  download: [file: FileMeta]
  versions: [file: FileMeta]
  rename: [file: FileMeta]
  delete: [file: FileMeta]
  move: [file: FileMeta]
  upload: []
  'toggle-select': [id: number]
  'select-all': [checked: boolean]
}>()
</script>

<template>
  <div class="list-wrap">
    <div class="empty" v-if="files.length === 0">此目录为空，点击右上角「上传」添加文件</div>
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
          :class="{ previewing: f.id === previewingId }"
          @click="emit('preview', f)"
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
  </div>
</template>

<style scoped>
.list-wrap {
  height: 100%;
  overflow: auto;
}
.empty {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--text-dim);
}
.file-table {
  width: 100%;
  min-width: 760px;
  border-collapse: collapse;
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
.file-table tbody tr.previewing .fname {
  color: var(--accent);
}
.fname {
  min-width: 140px;
  max-width: 320px;
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
</style>