export function formatBytes(n: number): string {
  if (n === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.min(Math.floor(Math.log(n) / Math.log(1024)), units.length - 1)
  return `${(n / Math.pow(1024, i)).toFixed(i === 0 ? 0 : 1)} ${units[i]}`
}

export function formatDate(s: string): string {
  if (!s) return '-'
  return s.replace('T', ' ').slice(0, 19)
}

const IMAGE_EXTS = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'svg', 'ico']
const CAD_EXTS = ['dwg', 'dxf', 'step', 'stp', 'stl', 'iges', 'igs', '3mf']
const TEXT_EXTS = ['txt', 'csv', 'log', 'json', 'ini', 'cfg', 'conf', 'md', 'xml', 'html', 'htm', 'py', 'js', 'ts', 'c', 'h', 'rs']
const ARCHIVE_EXTS = ['zip', 'rar']

export function isImage(ext: string): boolean {
  return IMAGE_EXTS.includes(ext.toLowerCase())
}
export function isCad(ext: string): boolean {
  return CAD_EXTS.includes(ext.toLowerCase())
}
export function isText(ext: string): boolean {
  return TEXT_EXTS.includes(ext.toLowerCase())
}
export function isArchive(ext: string): boolean {
  return ARCHIVE_EXTS.includes(ext.toLowerCase())
}

export function fileBadge(ext: string): { t: string; c: string } {
  const e = ext.toLowerCase()
  if (e === 'dwg') return { t: 'DWG', c: 'red' }
  if (e === 'dxf') return { t: 'DXF', c: 'orange' }
  if (e === 'step' || e === 'stp') return { t: 'STEP', c: 'blue' }
  if (e === 'stl') return { t: 'STL', c: 'cyan' }
  if (e === '3mf') return { t: '3MF', c: 'cyan' }
  if (e === 'iges' || e === 'igs') return { t: 'IGES', c: 'blue' }
  if (e === 'pdf') return { t: 'PDF', c: 'magenta' }
  if (isImage(e)) return { t: e.slice(0, 4).toUpperCase(), c: 'green' }
  return { t: (e || 'FILE').slice(0, 4).toUpperCase(), c: 'gray' }
}

export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

export async function saveBlob(name: string, blob: Blob): Promise<boolean> {
  if (isTauri()) {
    const { save } = await import('@tauri-apps/plugin-dialog')
    const { writeFile } = await import('@tauri-apps/plugin-fs')
    const path = await save({ defaultPath: name })
    if (!path) return false
    const bytes = new Uint8Array(await blob.arrayBuffer())
    await writeFile(path, bytes)
    return true
  } else {
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = name
    a.click()
    setTimeout(() => URL.revokeObjectURL(url), 5000)
    return true
  }
}

// 递归读取文件夹内容，返回所有File对象（保留webkitRelativePath路径）
async function readDirectoryEntries(directory: FileSystemDirectoryEntry): Promise<File[]> {
  const files: File[] = []
  const reader = directory.createReader()
  // readEntries 可能一次读不完所有条目，需要循环读取直到返回空数组
  let entries: FileSystemEntry[] = []
  do {
    entries = await new Promise<FileSystemEntry[]>((resolve, reject) => {
      reader.readEntries(resolve, reject)
    })
    for (const entry of entries) {
      if (entry.isFile) {
        const file = await new Promise<File>((resolve, reject) => {
          (entry as FileSystemFileEntry).file(resolve, reject)
        })
        // 保留webkitRelativePath路径信息
        if (directory.fullPath !== '/') {
          file._webkitRelativePath = directory.fullPath.substring(1) + '/' + file.name
        }
        files.push(file)
      } else if (entry.isDirectory) {
        const subFiles = await readDirectoryEntries(entry as FileSystemDirectoryEntry)
        files.push(...subFiles)
      }
    }
  } while (entries.length > 0)
  return files
}

// 从拖拽事件中提取文件（支持文件夹递归读取）
export async function getFilesFromDropEvent(e: DragEvent): Promise<File[]> {
  const dt = e.dataTransfer
  if (!dt) return []
  
  // 优先使用DataTransferItem API（支持文件夹）
  if (dt.items && dt.items.length > 0) {
    const files: File[] = []
    const entries: FileSystemEntry[] = []
    
    // 收集所有FileSystemEntry
    for (let i = 0; i < dt.items.length; i++) {
      const item = dt.items[i]
      if (item.kind === 'file') {
        const entry = item.webkitGetAsEntry?.()
        if (entry) entries.push(entry)
      }
    }
    
    for (const entry of entries) {
      if (entry.isFile) {
        const file = await new Promise<File>((resolve, reject) => {
          (entry as FileSystemFileEntry).file(resolve, reject)
        })
        files.push(file)
      } else if (entry.isDirectory) {
        const subFiles = await readDirectoryEntries(entry as FileSystemDirectoryEntry)
        files.push(...subFiles)
      }
    }
    if (files.length > 0) return files
  }
  
  // 回退到DataTransfer.files（仅支持文件，不支持文件夹）
  return Array.from(dt.files)
}

// 为File对象添加webkitRelativePath属性的类型扩展
declare global {
  interface File {
    _webkitRelativePath?: string
  }
}
