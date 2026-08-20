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

export function isImage(ext: string): boolean {
  return IMAGE_EXTS.includes(ext.toLowerCase())
}
export function isCad(ext: string): boolean {
  return CAD_EXTS.includes(ext.toLowerCase())
}
export function isText(ext: string): boolean {
  return TEXT_EXTS.includes(ext.toLowerCase())
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

export async function saveBlob(name: string, blob: Blob): Promise<void> {
  if (isTauri()) {
    const { save } = await import('@tauri-apps/plugin-dialog')
    const { writeFile } = await import('@tauri-apps/plugin-fs')
    const path = await save({ defaultPath: name })
    if (!path) return
    const bytes = new Uint8Array(await blob.arrayBuffer())
    await writeFile(path, bytes)
  } else {
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = name
    a.click()
    setTimeout(() => URL.revokeObjectURL(url), 5000)
  }
}
