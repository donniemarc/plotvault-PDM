<script setup lang="ts">
import { ref, watch, computed, onMounted, onBeforeUnmount } from 'vue'
import type { Folder, FileMeta, VersionInfo } from '../types'
import { api, getConfig } from '../api'
import PreviewPane from '../preview/PreviewPane.vue'

const props = defineProps<{
  folder: Folder | null
  file: FileMeta | null
  activeTab: 'property' | 'filelist' | 'versions' | 'related' | 'history'
  showInlinePreview?: boolean
}>()

const emit = defineEmits<{
  (e: 'update:activeTab', tab: 'property' | 'filelist' | 'versions' | 'related' | 'history'): void
  (e: 'update:folder', folder: Folder): void
  (e: 'update:file', file: FileMeta): void
  (e: 'toast', msg: string, type: string): void
  (e: 'select-folder', id: number): void
  (e: 'preview', file: FileMeta): void
  (e: 'download', file: FileMeta): void
  (e: 'delete', file: FileMeta): void
  (e: 'rename', file: FileMeta): void
  (e: 'upload'): void
}>()

function onGlobalClick() {
  closeFileContextMenu()
}
onMounted(() => document.addEventListener('click', onGlobalClick))
onBeforeUnmount(() => document.removeEventListener('click', onGlobalClick))

// 表单数据
const formData = ref({
  name: '',
  code: '',
  stage: '',
  status: '',
  description: '',
  remarks: '',
  creator: '',
  drawing_size: '',
  source_file_type: '',
  source_file_version: '',
  other_info: '',
  publish_time: '',
})

// 原始数据（用于比较是否有修改）
const originalData = ref({ ...formData.value })

// 是否有修改
const hasChanges = computed(() => {
  return JSON.stringify(formData.value) !== JSON.stringify(originalData.value)
})

// 是否正在保存
const saving = ref(false)

// 阶段选项
const stageOptions = ['草案', '审核中', '已批准', '已发布', '已归档']

// 状态选项
const statusOptions = ['修改中', '待审核', '已审核', '已确认', '已废弃']

// 图幅选项
const drawingSizeOptions = ['A0', 'A1', 'A2', 'A3', 'A4', '自定义']

// 源文件类型选项
const sourceFileTypeOptions = ['DWG', 'DXF', 'STEP', 'STP', 'IGES', 'IGS', 'STL', '3MF', 'PDF', '其他']

// 文件列表标签页数据
const childFolders = ref<Folder[]>([])
const childFiles = ref<FileMeta[]>([])
const loadingFiles = ref(false)
const showAllChildren = ref(true)
const showFilesOnly = ref(false)
const selectedFileIds = ref<Set<number>>(new Set())

// 全选状态
const allFilesSelected = computed(() => {
  return filteredChildFiles.value.length > 0 && filteredChildFiles.value.every(f => selectedFileIds.value.has(f.id))
})
const someFilesSelected = computed(() => {
  return filteredChildFiles.value.some(f => selectedFileIds.value.has(f.id))
})

function toggleFileSelect(id: number) {
  const s = new Set(selectedFileIds.value)
  if (s.has(id)) s.delete(id)
  else s.add(id)
  selectedFileIds.value = s
}

function toggleSelectAllFiles() {
  if (allFilesSelected.value) {
    selectedFileIds.value = new Set()
  } else {
    selectedFileIds.value = new Set(filteredChildFiles.value.map(f => f.id))
  }
}

// 文件列表右键菜单
const fileContextMenu = ref<{ x: number; y: number; file: FileMeta } | null>(null)

function onFileContextMenu(e: MouseEvent, file: FileMeta) {
  e.preventDefault()
  e.stopPropagation()
  const menuHeight = 160
  const viewportHeight = window.innerHeight
  let y = e.clientY
  if (y + menuHeight > viewportHeight) {
    y = Math.max(0, viewportHeight - menuHeight - 10)
  }
  let x = e.clientX
  const menuWidth = 180
  if (x + menuWidth > window.innerWidth) {
    x = window.innerWidth - menuWidth - 10
  }
  fileContextMenu.value = { x, y, file }
}

function closeFileContextMenu() {
  fileContextMenu.value = null
}

function fileCtxAction(action: string) {
  if (!fileContextMenu.value) return
  const file = fileContextMenu.value.file
  closeFileContextMenu()
  if (action === 'download') emit('download', file)
  else if (action === 'preview') emit('preview', file)
  else if (action === 'rename') emit('rename', file)
  else if (action === 'delete') emit('delete', file)
}

// 筛选后的子项
const filteredChildFolders = computed(() => {
  if (showFilesOnly.value) return []
  return childFolders.value
})

const filteredChildFiles = computed(() => {
  return childFiles.value
})

// 版本控制标签页数据
const versions = ref<VersionInfo[]>([])
const loadingVersions = ref(false)
const uploadingVersion = ref(false)

// 监听props变化，更新表单数据
watch(() => props.folder, (newFolder) => {
  if (newFolder) {
    formData.value = {
      name: newFolder.name,
      code: newFolder.code || '',
      stage: newFolder.stage || '',
      status: newFolder.status || '',
      description: newFolder.description || '',
      remarks: newFolder.remarks || '',
      creator: newFolder.creator || '',
      drawing_size: '',
      source_file_type: '',
      source_file_version: '',
      other_info: '',
      publish_time: '',
    }
    originalData.value = { ...formData.value }
  }
}, { immediate: true })

watch(() => props.file, (newFile) => {
  if (newFile) {
    formData.value = {
      name: newFile.name,
      code: newFile.code || '',
      stage: newFile.stage || '',
      status: newFile.status || '',
      description: newFile.description || '',
      remarks: newFile.remarks || '',
      creator: newFile.creator || '',
      drawing_size: newFile.drawing_size || '',
      source_file_type: newFile.source_file_type || '',
      source_file_version: newFile.source_file_version || '',
      other_info: newFile.other_info || '',
      publish_time: newFile.publish_time || '',
    }
    originalData.value = { ...formData.value }
  }
}, { immediate: true })

// 监听标签页变化，加载数据
watch(() => props.activeTab, async (tab) => {
  if (tab === 'filelist' && props.folder) {
    await loadChildren()
  } else if (tab === 'versions' && props.file) {
    await loadVersions()
  }
}, { immediate: true })

// 加载子文件夹和文件
async function loadChildren() {
  if (!props.folder) return
  loadingFiles.value = true
  try {
    const tree = await api.tree()
    childFolders.value = tree.folders.filter(f => f.parent_id === props.folder!.id)
    childFiles.value = tree.files.filter(f => f.folder_id === props.folder!.id)
  } catch (e: any) {
    emit('toast', e.message || '加载文件列表失败', 'error')
  } finally {
    loadingFiles.value = false
  }
}

// 加载版本历史
async function loadVersions() {
  if (!props.file) return
  loadingVersions.value = true
  try {
    const result = await api.versions(props.file.id)
    versions.value = result.versions
  } catch (e: any) {
    emit('toast', e.message || '加载版本历史失败', 'error')
  } finally {
    loadingVersions.value = false
  }
}

// 上传新版本
async function uploadNewVersion() {
  if (!props.file) return
  const input = document.createElement('input')
  input.type = 'file'
  input.onchange = async (e) => {
    const file = (e.target as HTMLInputElement).files?.[0]
    if (!file) return
    uploadingVersion.value = true
    try {
      await api.addVersion(props.file!.id, file, '')
      emit('toast', '新版本上传成功', 'ok')
      await loadVersions()
      // 更新文件信息
      const tree = await api.tree()
      const updatedFile = tree.files.find(f => f.id === props.file!.id)
      if (updatedFile) {
        emit('update:file', updatedFile)
      }
    } catch (e: any) {
      emit('toast', e.message || '上传失败', 'error')
    } finally {
      uploadingVersion.value = false
    }
  }
  input.click()
}

// 下载指定版本
function downloadVersion(version: VersionInfo) {
  if (!props.file) return
  const config = getConfig()
  const link = document.createElement('a')
  link.href = `${config.url}/api/files/${props.file.id}/download?version=${version.version_no}`
  if (config.token) {
    link.href += `&token=${config.token}`
  }
  link.download = `${props.file.name.replace(/\.[^.]+$/, '')}_v${version.version_no}.${props.file.ext}`
  link.click()
}

// 下载当前版本
function downloadCurrentVersion() {
  if (!props.file || versions.value.length === 0) return
  const currentVersion = versions.value.find(v => v.version_no === props.file!.current_version)
  if (currentVersion) {
    downloadVersion(currentVersion)
  }
}

// 删除版本（预留）
function deleteVersion() {
  if (!props.file || versions.value.length <= 1) return
  emit('toast', '删除版本功能暂未实现', 'info')
}

// 回滚版本（预留）
function rollbackVersion() {
  if (!props.file || versions.value.length <= 1) return
  emit('toast', '回滚版本功能暂未实现', 'info')
}

// 保存属性
async function saveProps() {
  if (!hasChanges.value) return
  
  saving.value = true
  try {
    if (props.folder) {
      const updated = await api.updateFolderProps(props.folder.id, {
        name: formData.value.name,
        code: formData.value.code,
        stage: formData.value.stage,
        status: formData.value.status,
        description: formData.value.description,
        remarks: formData.value.remarks,
        creator: formData.value.creator,
      })
      emit('update:folder', updated)
      emit('toast', '文件夹属性已保存', 'ok')
    } else if (props.file) {
      const updated = await api.patchFile(props.file.id, {
        name: formData.value.name,
        code: formData.value.code,
        stage: formData.value.stage,
        status: formData.value.status,
        description: formData.value.description,
        remarks: formData.value.remarks,
        creator: formData.value.creator,
        drawing_size: formData.value.drawing_size,
        source_file_type: formData.value.source_file_type,
        source_file_version: formData.value.source_file_version,
        other_info: formData.value.other_info,
        publish_time: formData.value.publish_time,
      })
      emit('update:file', updated)
      emit('toast', '文件属性已保存', 'ok')
    }
    originalData.value = { ...formData.value }
  } catch (e: any) {
    emit('toast', e.message || '保存失败', 'error')
  } finally {
    saving.value = false
  }
}

// 重置表单
function resetForm() {
  formData.value = { ...originalData.value }
}

// 格式化文件大小
function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i]
}

// 获取文件类型标签
function getFileTypeLabel(ext: string): string {
  const types: Record<string, string> = {
    'dwg': 'DWG 图纸',
    'dxf': 'DXF 图纸',
    'step': 'STEP 模型',
    'stp': 'STEP 模型',
    'iges': 'IGES 模型',
    'igs': 'IGES 模型',
    'stl': 'STL 模型',
    '3mf': '3MF 模型',
    'pdf': 'PDF 文档',
    'png': 'PNG 图片',
    'jpg': 'JPG 图片',
    'jpeg': 'JPG 图片',
    'txt': '文本文件',
  }
  return types[ext.toLowerCase()] || ext.toUpperCase()
}

// 获取文件类型图标
function getFileTypeIcon(ext: string): string {
  const icons: Record<string, string> = {
    'dwg': '📐',
    'dxf': '📐',
    'step': '📦',
    'stp': '📦',
    'iges': '📦',
    'igs': '📦',
    'stl': '📦',
    '3mf': '📦',
    'pdf': '📄',
    'png': '🖼️',
    'jpg': '🖼️',
    'jpeg': '🖼️',
    'txt': '📝',
  }
  return icons[ext.toLowerCase()] || '📄'
}
</script>

<template>
  <div class="property-panel" @mousedown.middle.prevent>
    <!-- 标签页头部 -->
    <div class="tabs-header">
      <button 
        class="tab-btn" 
        :class="{ active: activeTab === 'property' }"
        @click="emit('update:activeTab', 'property')"
      >属性</button>
      <button 
        v-if="folder"
        class="tab-btn" 
        :class="{ active: activeTab === 'filelist' }"
        @click="emit('update:activeTab', 'filelist')"
      >文件列表</button>
      <button 
        v-if="file"
        class="tab-btn" 
        :class="{ active: activeTab === 'versions' }"
        @click="emit('update:activeTab', 'versions')"
      >版本控制</button>
      <button 
        class="tab-btn" 
        :class="{ active: activeTab === 'related' }"
        @click="emit('update:activeTab', 'related')"
      >关联文档</button>
      <button 
        class="tab-btn" 
        :class="{ active: activeTab === 'history' }"
        @click="emit('update:activeTab', 'history')"
      >历史任务</button>
    </div>

    <!-- 属性标签页内容 -->
    <div v-if="activeTab === 'property'" class="tab-content">
      <!-- 文件夹属性 -->
      <template v-if="folder">
        <div class="prop-form">
          <div class="prop-row">
            <label class="prop-label">编码</label>
            <input v-model="formData.code" type="text" class="prop-input" placeholder="自动生成" />
          </div>
          <div class="prop-row">
            <label class="prop-label">名称</label>
            <input v-model="formData.name" type="text" class="prop-input" />
          </div>
          <div class="prop-row">
            <label class="prop-label">目录类型</label>
            <input type="text" class="prop-input" value="文件夹" disabled />
          </div>
          <div class="prop-row">
            <label class="prop-label">阶段</label>
            <select v-model="formData.stage" class="prop-input">
              <option value="">未设置</option>
              <option v-for="opt in stageOptions" :key="opt" :value="opt">{{ opt }}</option>
            </select>
          </div>
          <div class="prop-row">
            <label class="prop-label">状态</label>
            <select v-model="formData.status" class="prop-input">
              <option value="">未设置</option>
              <option v-for="opt in statusOptions" :key="opt" :value="opt">{{ opt }}</option>
            </select>
          </div>
          <div class="prop-row">
            <label class="prop-label">创建时间</label>
            <input type="text" class="prop-input" :value="folder.created_at" disabled />
          </div>
          <div class="prop-row">
            <label class="prop-label">修改时间</label>
            <input type="text" class="prop-input" :value="folder.created_at" disabled />
          </div>
          <div class="prop-row full">
            <label class="prop-label">描述</label>
            <textarea v-model="formData.description" class="prop-textarea" rows="3" placeholder="输入描述信息" @mousedown.middle.prevent></textarea>
          </div>
        </div>
      </template>

      <!-- 文件属性 -->
      <template v-else-if="file">
        <!-- 内嵌预览区域（放在最顶部） -->
        <div v-if="showInlinePreview" class="inline-preview">
          <div class="inline-preview-header">
            <span class="inline-preview-title">预览</span>
            <span class="dim">v{{ file.current_version }}</span>
          </div>
          <div class="inline-preview-body">
            <PreviewPane :file="file" />
          </div>
        </div>

        <div class="prop-form">
          <div class="prop-row">
            <label class="prop-label">编码</label>
            <input v-model="formData.code" type="text" class="prop-input" placeholder="自动生成" />
          </div>
          <div class="prop-row">
            <label class="prop-label">名称</label>
            <input v-model="formData.name" type="text" class="prop-input" />
          </div>
          <div class="prop-row">
            <label class="prop-label">文件类型</label>
            <input type="text" class="prop-input" :value="getFileTypeLabel(file.ext)" disabled />
          </div>
          <div class="prop-row">
            <label class="prop-label">阶段</label>
            <select v-model="formData.stage" class="prop-input">
              <option value="">未设置</option>
              <option v-for="opt in stageOptions" :key="opt" :value="opt">{{ opt }}</option>
            </select>
          </div>
          <div class="prop-row">
            <label class="prop-label">版本</label>
            <input type="text" class="prop-input" :value="'v' + file.current_version" disabled />
          </div>
          <div class="prop-row">
            <label class="prop-label">状态</label>
            <select v-model="formData.status" class="prop-input">
              <option value="">未设置</option>
              <option v-for="opt in statusOptions" :key="opt" :value="opt">{{ opt }}</option>
            </select>
          </div>
          <div class="prop-row">
            <label class="prop-label">图幅</label>
            <select v-model="formData.drawing_size" class="prop-input">
              <option value="">未设置</option>
              <option v-for="opt in drawingSizeOptions" :key="opt" :value="opt">{{ opt }}</option>
            </select>
          </div>
          <div class="prop-row">
            <label class="prop-label">源文件版本</label>
            <input v-model="formData.source_file_version" type="text" class="prop-input" placeholder="输入源文件版本" />
          </div>
          <div class="prop-row">
            <label class="prop-label">生成时间</label>
            <input type="text" class="prop-input" :value="file.created_at" disabled />
          </div>
          <div class="prop-row">
            <label class="prop-label">修改时间</label>
            <input type="text" class="prop-input" :value="file.updated_at" disabled />
          </div>
          <div class="prop-row">
            <label class="prop-label">发布时间</label>
            <input v-model="formData.publish_time" type="text" class="prop-input" placeholder="YYYY-MM-DD HH:mm:ss" />
          </div>
          <div class="prop-row">
            <label class="prop-label">文件大小</label>
            <input type="text" class="prop-input" :value="formatBytes(file.size)" disabled />
          </div>
          <div class="prop-row full">
            <label class="prop-label">描述</label>
            <textarea v-model="formData.description" class="prop-textarea" rows="3" placeholder="输入描述信息" @mousedown.middle.prevent></textarea>
          </div>
        </div>
      </template>

      <!-- 无选中项 -->
      <template v-else>
        <div class="empty-state">
          <div class="empty-icon">📁</div>
          <p>请选择文件夹或文件查看属性</p>
        </div>
      </template>

      <!-- 操作按钮 -->
      <div v-if="folder || file" class="action-buttons">
        <!-- 第1组：常用操作 -->
        <div class="button-group">
          <div class="group-buttons">
            <button v-if="folder" class="btn-action" @click="emit('upload')">上传文件</button>
            <button v-if="file" class="btn-action" @click="emit('download', file)">下载文件</button>
            <button v-if="file" class="btn-action" @click="emit('rename', file)">重命名</button>
            <button v-if="file" class="btn-danger" @click="emit('delete', file)">删除</button>
            <button class="btn-save" :disabled="!hasChanges || saving" @click="saveProps">
              {{ saving ? '保存中...' : '保存' }}
            </button>
            <button class="btn-action" @click="resetForm" :disabled="!hasChanges">重置</button>
          </div>
        </div>
      </div>
    </div>

    <!-- 文件列表标签页 -->
    <div v-if="activeTab === 'filelist' && folder" class="tab-content">
      <div class="filelist-toolbar">
        <button class="btn-action" @click="emit('upload')">上传文件</button>
        <div class="toolbar-separator"></div>
        <label class="filter-option">
          <input type="checkbox" v-model="showAllChildren" />
          <span>显示所有子文件</span>
        </label>
        <label class="filter-option">
          <input type="checkbox" v-model="showFilesOnly" />
          <span>只显示文件</span>
        </label>
      </div>
      <div v-if="loadingFiles" class="loading-state">
        <span class="loading-icon">⏳</span>
        <span>加载中...</span>
      </div>
      <div v-else-if="filteredChildFolders.length === 0 && filteredChildFiles.length === 0" class="empty-state">
        <div class="empty-icon">📂</div>
        <p>此文件夹为空</p>
      </div>
      <div v-else class="files-table" @contextmenu.prevent>
        <div class="table-header">
          <div class="col-check">
            <input type="checkbox" :checked="allFilesSelected" :indeterminate="someFilesSelected && !allFilesSelected" @change="toggleSelectAllFiles" />
          </div>
          <div class="col-name">名称</div>
          <div class="col-type">类型</div>
          <div class="col-size">大小 (Kb)</div>
          <div class="col-time">修改时间</div>
        </div>
        <!-- 子文件夹 -->
        <div v-for="f in filteredChildFolders" :key="'folder-' + f.id" class="table-row folder-item" @click="emit('select-folder', f.id)">
          <div class="col-check"></div>
          <div class="col-name">
            <span class="file-icon">📁</span>
            <span class="file-name">{{ f.name }}</span>
          </div>
          <div class="col-type">目录</div>
          <div class="col-size">—</div>
          <div class="col-time">{{ f.created_at }}</div>
        </div>
        <!-- 文件 -->
        <div v-for="f in filteredChildFiles" :key="'file-' + f.id" class="table-row" @contextmenu="onFileContextMenu($event, f)">
          <div class="col-check" @click.stop>
            <input type="checkbox" :checked="selectedFileIds.has(f.id)" @change="toggleFileSelect(f.id)" />
          </div>
          <div class="col-name">
            <span class="file-icon">{{ getFileTypeIcon(f.ext) }}</span>
            <span class="file-name no-jump">{{ f.name }}</span>
          </div>
          <div class="col-type">{{ f.ext.toUpperCase() }}</div>
          <div class="col-size">{{ (f.size / 1024).toFixed(1) }}</div>
          <div class="col-time">{{ f.updated_at || f.created_at }}</div>
        </div>
      </div>

      <!-- 文件右键菜单 -->
      <Teleport to="body">
        <div
          v-if="fileContextMenu"
          class="file-ctx-menu"
          :style="{ left: fileContextMenu.x + 'px', top: fileContextMenu.y + 'px' }"
          @click.stop
        >
          <div class="ctx-item" @click="fileCtxAction('preview')">
            <span class="ctx-ico">👁</span> 预览
          </div>
          <div class="ctx-sep" />
          <div class="ctx-item" @click="fileCtxAction('download')">
            <span class="ctx-ico">⬇</span> 下载
          </div>
          <div class="ctx-sep" />
          <div class="ctx-item" @click="fileCtxAction('rename')">
            <span class="ctx-ico">✎</span> 重命名
          </div>
          <div class="ctx-sep" />
          <div class="ctx-item ctx-danger" @click="fileCtxAction('delete')">
            <span class="ctx-ico">🗑</span> 删除
          </div>
        </div>
      </Teleport>
    </div>

    <!-- 版本控制标签页 -->
    <div v-if="activeTab === 'versions' && file" class="tab-content">
      <div class="versions-toolbar">
        <button class="btn-action" :disabled="uploadingVersion" @click="uploadNewVersion">
          {{ uploadingVersion ? '上传中...' : '创建小版本' }}
        </button>
        <button class="btn-action" :disabled="versions.length === 0" @click="downloadCurrentVersion">下载小版本</button>
        <button class="btn-action" :disabled="versions.length <= 1" @click="deleteVersion">删除小版本</button>
        <button class="btn-action" :disabled="versions.length <= 1" @click="rollbackVersion">回滚到小版本</button>
      </div>
      <div v-if="loadingVersions" class="loading-state">
        <span class="loading-icon">⏳</span>
        <span>加载中...</span>
      </div>
      <div v-else-if="versions.length === 0" class="empty-state">
        <div class="empty-icon">📋</div>
        <p>暂无版本历史</p>
      </div>
      <div v-else class="versions-table">
        <div class="table-header">
          <div class="col-version">版本</div>
          <div class="col-creator">创建人</div>
          <div class="col-time">创建时间</div>
          <div class="col-status">文件状态</div>
          <div class="col-desc">描述</div>
          <div class="col-actions">操作</div>
        </div>
        <div v-for="v in versions" :key="v.id" class="table-row" :class="{ current: v.version_no === file.current_version }">
          <div class="col-version">v{{ v.version_no }}</div>
          <div class="col-creator">{{ file.creator || '—' }}</div>
          <div class="col-time">{{ v.created_at }}</div>
          <div class="col-status">{{ v.version_no === file.current_version ? '当前' : '历史' }}</div>
          <div class="col-desc">{{ v.comment || '—' }}</div>
          <div class="col-actions">
            <button class="btn-link" @click="downloadVersion(v)">下载</button>
          </div>
        </div>
      </div>
    </div>

    <!-- 关联文档标签页 -->
    <div v-if="activeTab === 'related'" class="tab-content">
      <div class="empty-state">
        <div class="empty-icon">🔗</div>
        <p>关联文档功能将在后续实现</p>
      </div>
    </div>

    <!-- 历史任务标签页 -->
    <div v-if="activeTab === 'history'" class="tab-content">
      <div class="history-toolbar">
        <button class="btn-action" disabled>查看任务</button>
      </div>
      <div class="history-table">
        <div class="table-header">
          <div class="col-taskcode">任务代号</div>
          <div class="col-taskname">任务名称</div>
          <div class="col-tasktype">任务类别</div>
          <div class="col-flow">执行流程</div>
          <div class="col-status">任务状态</div>
          <div class="col-createtime">生成时间</div>
          <div class="col-endtime">结束时间</div>
          <div class="col-creator">生成人</div>
          <div class="col-owner">负责人</div>
          <div class="col-project">相关项目</div>
          <div class="col-desc">描述</div>
        </div>
        <div class="empty-table-row">
          <div style="grid-column: 1 / -1; text-align: center; padding: 24px; color: var(--text-dim);">
            暂无历史任务
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.property-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--bg-panel);
}

.tabs-header {
  display: flex;
  border-bottom: 1px solid var(--border);
  padding: 0 12px;
  background: var(--bg);
}

.tab-btn {
  padding: 10px 16px;
  border: none;
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
  font-size: var(--font-base);
  border-bottom: 2px solid transparent;
  transition: all var(--transition-fast);
}

.tab-btn:hover {
  color: var(--text);
}

.tab-btn.active {
  color: var(--accent);
  border-bottom-color: var(--accent);
}

.tab-content {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
}

.prop-form {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.prop-row {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.prop-row.full {
  grid-column: 1 / -1;
}

.prop-label {
  width: 80px;
  flex-shrink: 0;
  color: var(--text-dim);
  font-size: var(--font-sm);
}

.prop-input {
  flex: 1;
  padding: 6px 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg);
  color: var(--text);
  font-size: var(--font-base);
}

.prop-input:disabled {
  background: var(--bg-elevated);
  color: var(--text-dim);
  cursor: not-allowed;
}

.prop-textarea {
  flex: 1;
  padding: 6px 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg);
  color: var(--text);
  font-size: var(--font-base);
  resize: vertical;
  min-height: 60px;
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--text-dim);
}

.empty-icon {
  font-size: 48px;
  margin-bottom: 16px;
}

.action-buttons {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  padding: 16px;
  border-top: 1px solid var(--border);
  margin-top: auto;
}

.btn-save {
  flex: 1;
  padding: 8px 16px;
  background: var(--accent);
  color: var(--text-on-accent);
  border: none;
  border-radius: var(--radius-sm);
  cursor: pointer;
  font-size: var(--font-base);
}

.btn-save:hover:not(:disabled) {
  background: var(--accent-hover);
}

.btn-save:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-reset {
  padding: 8px 16px;
  background: transparent;
  color: var(--text-dim);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  cursor: pointer;
  font-size: var(--font-base);
}

.btn-reset:hover:not(:disabled) {
  background: var(--bg-hover);
}

.btn-reset:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-action {
  padding: 8px 12px;
  background: transparent;
  color: var(--text);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  cursor: pointer;
  font-size: var(--font-base);
}

.btn-action:hover {
  background: var(--bg-hover);
}

.btn-danger {
  padding: 8px 12px;
  background: transparent;
  color: var(--danger);
  border: 1px solid var(--danger);
  border-radius: var(--radius-sm);
  cursor: pointer;
  font-size: var(--font-base);
}

.btn-danger:hover {
  background: var(--danger-soft);
}

.loading-state {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  height: 100%;
  color: var(--text-dim);
}

.loading-icon {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.files-list,
.versions-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.file-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background var(--transition-fast);
}

.file-item:hover {
  background: var(--bg-hover);
}

.file-item.folder-item {
  font-weight: 500;
}

.file-icon {
  font-size: 16px;
  width: 20px;
  text-align: center;
}

.file-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-meta {
  color: var(--text-dim);
  font-size: var(--font-sm);
  white-space: nowrap;
}

.versions-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}

.versions-header h4 {
  margin: 0;
  font-size: var(--font-base);
  color: var(--text);
}

.btn-upload-version {
  padding: 6px 12px;
  background: var(--accent);
  color: var(--text-on-accent);
  border: none;
  border-radius: var(--radius-sm);
  cursor: pointer;
  font-size: var(--font-sm);
}

.btn-upload-version:hover:not(:disabled) {
  background: var(--accent-hover);
}

.btn-upload-version:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.version-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg);
}

.version-item.current {
  border-color: var(--accent);
  background: var(--accent-soft);
}

.version-info {
  display: flex;
  align-items: center;
  gap: 12px;
}

.version-no {
  font-weight: 600;
  color: var(--text);
}

.version-size {
  color: var(--text-dim);
  font-size: var(--font-sm);
}

.version-time {
  color: var(--text-dim);
  font-size: var(--font-sm);
  margin-left: auto;
}

.version-comment {
  color: var(--text-dim);
  font-size: var(--font-sm);
}

.version-actions {
  display: flex;
  justify-content: flex-end;
  margin-top: 4px;
}

.btn-download {
  padding: 4px 8px;
  background: transparent;
  color: var(--accent);
  border: 1px solid var(--accent);
  border-radius: var(--radius-sm);
  cursor: pointer;
  font-size: var(--font-xs);
}

.btn-download:hover {
  background: var(--accent-soft);
}

/* 按钮组样式 */
.action-buttons {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 12px 16px;
  border-top: 1px solid var(--border);
  margin-top: auto;
}

.button-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.group-label {
  font-size: var(--font-xs);
  color: var(--text-dim);
  font-weight: 500;
}

.group-buttons {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

/* 文件列表工具栏 */
.filelist-toolbar,
.versions-toolbar,
.history-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--border);
  margin-bottom: 12px;
  flex-wrap: wrap;
}

.toolbar-separator {
  width: 1px;
  height: 20px;
  background: var(--border);
  margin: 0 4px;
}

.filter-option {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: var(--font-sm);
  color: var(--text-dim);
  cursor: pointer;
}

.filter-option input {
  cursor: pointer;
}

/* 表格样式 */
.files-table,
.versions-table,
.history-table {
  display: flex;
  flex-direction: column;
  font-size: var(--font-sm);
}

.table-header {
  display: grid;
  gap: 8px;
  padding: 8px 12px;
  background: var(--bg-elevated);
  border-radius: var(--radius-sm);
  font-weight: 500;
  color: var(--text-dim);
}

.files-table .table-header {
  grid-template-columns: 28px 1fr 60px 80px 120px;
}

.versions-table .table-header {
  grid-template-columns: 60px 80px 120px 60px 1fr 60px;
}

.history-table .table-header {
  grid-template-columns: 80px 80px 60px 60px 60px 100px 100px 60px 60px 80px 1fr;
  font-size: var(--font-xs);
}

.table-row {
  display: grid;
  gap: 8px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--border);
  cursor: pointer;
  transition: background var(--transition-fast);
  align-items: center;
}

.table-row:hover {
  background: var(--bg-hover);
}

.table-row.current {
  background: var(--accent-soft);
}

.files-table .table-row {
  grid-template-columns: 28px 1fr 60px 80px 120px;
}

.versions-table .table-row {
  grid-template-columns: 60px 80px 120px 60px 1fr 60px;
}

.history-table .table-row {
  grid-template-columns: 80px 80px 60px 60px 60px 100px 100px 60px 60px 80px 1fr;
  font-size: var(--font-xs);
}

.col-check {
  display: flex;
  align-items: center;
  justify-content: center;
}

.col-check input[type="checkbox"] {
  cursor: pointer;
  width: 14px;
  height: 14px;
}

.col-name {
  display: flex;
  align-items: center;
  gap: 8px;
  overflow: hidden;
}

.col-name .file-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.col-type,
.col-size,
.col-time,
.col-version,
.col-creator,
.col-status,
.col-desc,
.col-actions,
.col-taskcode,
.col-taskname,
.col-tasktype,
.col-flow,
.col-createtime,
.col-endtime,
.col-owner,
.col-project {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.btn-link {
  background: none;
  border: none;
  color: var(--accent);
  cursor: pointer;
  font-size: var(--font-xs);
  padding: 2px 4px;
}

.btn-link:hover {
  text-decoration: underline;
}

.empty-table-row {
  display: grid;
  grid-template-columns: 1fr;
}

/* 内嵌预览区域 */
.inline-preview {
  margin-top: 16px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  overflow: hidden;
}

.inline-preview-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: var(--bg-elevated);
  border-bottom: 1px solid var(--border);
}

.inline-preview-title {
  font-weight: 600;
  font-size: var(--font-sm);
  color: var(--text);
}

.dim {
  color: var(--text-dim);
  font-size: var(--font-sm);
}

.inline-preview-body {
  width: 100%;
  height: 400px;
  min-height: 300px;
  max-height: 60vh;
  overflow: hidden;
}

/* 文件列表右键菜单 */
.file-ctx-menu {
  position: fixed;
  z-index: 9999;
  min-width: 160px;
  background: var(--bg-panel);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  box-shadow: var(--shadow-md);
  padding: 4px 0;
}
.file-ctx-menu .ctx-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 14px;
  font-size: var(--font-sm);
  cursor: pointer;
  transition: background var(--transition-fast);
}
.file-ctx-menu .ctx-item:hover {
  background: var(--bg-hover);
}
.file-ctx-menu .ctx-ico {
  font-size: 13px;
  width: 18px;
  text-align: center;
}
.file-ctx-menu .ctx-sep {
  height: 1px;
  background: var(--border-faint);
  margin: 4px 0;
}
.file-ctx-menu .ctx-danger {
  color: var(--danger);
}

/* 文件名不跳转 */
.no-jump {
  cursor: default;
}
</style>
