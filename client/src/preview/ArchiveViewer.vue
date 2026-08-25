<script setup lang="ts">
import { computed } from 'vue'
import { formatBytes } from '../utils'

interface ArchiveEntry {
  path: string
  name: string
  is_dir: boolean
  size: number
}

const props = defineProps<{
  entries: ArchiveEntry[]
  fileId: number
  version?: number
}>()

interface FlatNode {
  name: string
  fullPath: string
  is_dir: boolean
  size: number
  depth: number
  childCount: number
}

const flatTree = computed(() => {
  const nodes: FlatNode[] = []
  const dirChildren = new Map<string, ArchiveEntry[]>()

  for (const entry of props.entries) {
    const parts = entry.path.split(/[/\\]/).filter(Boolean)
    if (parts.length <= 1) {
      nodes.push({
        name: entry.name,
        fullPath: entry.path,
        is_dir: entry.is_dir,
        size: entry.size,
        depth: 0,
        childCount: 0,
      })
    } else {
      const dirPath = parts.slice(0, -1).join('/')
      if (!dirChildren.has(dirPath)) dirChildren.set(dirPath, [])
      dirChildren.get(dirPath)!.push(entry)
    }
  }

  const expandedDirs = new Set<string>()

  function addDir(dirPath: string, depth: number) {
    const children = dirChildren.get(dirPath) || []
    const dirs = children.filter(c => c.is_dir)
    const files = children.filter(c => !c.is_dir)
    for (const d of dirs) {
      nodes.push({
        name: d.name,
        fullPath: d.path,
        is_dir: true,
        size: d.size,
        depth,
        childCount: (dirChildren.get(d.path.replace(/\/$/, '')) || []).length,
      })
    }
    for (const f of files) {
      nodes.push({
        name: f.name,
        fullPath: f.path,
        is_dir: false,
        size: f.size,
        depth,
        childCount: 0,
      })
    }
  }

  addDir('', 0)
  return nodes
})

const totalFiles = computed(() => props.entries.filter(e => !e.is_dir).length)
const totalSize = computed(() => props.entries.reduce((s, e) => s + e.size, 0))

function downloadEntry(path: string) {
  const base = window.location.origin
  const versionParam = props.version ? `&version=${props.version}` : ''
  const url = `${base}/api/files/${props.fileId}/archive-entry?path=${encodeURIComponent(path)}${versionParam}`
  window.open(url, '_blank')
}

function fileSize(node: FlatNode): string {
  if (node.is_dir) return `${node.childCount} 项`
  return formatBytes(node.size)
}
</script>

<template>
  <div class="archive-wrap">
    <div class="archive-header">
      <span class="archive-icon">&#128230;</span>
      <span>共 {{ totalFiles }} 个文件，{{ formatBytes(totalSize) }}</span>
    </div>
    <div class="archive-tree">
      <div
        v-for="node in flatTree"
        :key="node.fullPath"
        class="tree-row"
        :class="{ 'is-dir': node.is_dir }"
        :style="{ paddingLeft: (12 + node.depth * 20) + 'px' }"
        @click="node.is_dir ? undefined : downloadEntry(node.fullPath)"
      >
        <span v-if="node.is_dir" class="node-icon dir-icon">&#128193;</span>
        <span v-else class="node-icon file-icon">&#128196;</span>
        <span class="node-name">{{ node.name }}</span>
        <span class="node-size">{{ fileSize(node) }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.archive-wrap {
  width: 100%;
  height: 100%;
  overflow: auto;
  padding: 0;
}
.archive-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border);
  color: var(--text-dim);
  font-size: 13px;
  position: sticky;
  top: 0;
  background: var(--bg);
  z-index: 1;
}
.archive-icon {
  font-size: 18px;
}
.archive-tree {
  padding: 8px 0;
}
.tree-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 16px;
  font-size: 13px;
  color: var(--text);
  transition: background 0.1s;
}
.tree-row:not(.is-dir) {
  cursor: pointer;
}
.tree-row:not(.is-dir):hover {
  background: var(--hover);
}
.node-icon {
  font-size: 14px;
  flex-shrink: 0;
}
.dir-icon {
  font-size: 15px;
}
.file-icon {
  font-size: 14px;
}
.node-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.node-size {
  color: var(--text-dim);
  font-size: 12px;
  white-space: nowrap;
  margin-left: 8px;
}
</style>
