import type { FileMeta, Folder, Tree, VersionInfo } from './types'

const STORAGE_KEY = 'plotvault_pdm_config'
export const DEFAULT_SERVER = 'http://192.168.1.1:38642'

// 服务器端 STEP/IGES 转换服务地址（可选）。留空 = 本机 Web Worker 解析；
// 若 NAS 上部署了 converter 容器（见 server/converter/），填 http://<nas>:8000 可把解析压力放到服务器
export const STEP_CONVERT_URL = ''

export interface ServerConfig {
  url: string
  token: string
}

export function loadConfig(): ServerConfig {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw) {
      const c = JSON.parse(raw)
      return { url: c.url || DEFAULT_SERVER, token: c.token || '' }
    }
  } catch {
    /* ignore */
  }
  return { url: DEFAULT_SERVER, token: '' }
}

let cfg: ServerConfig = loadConfig()

export function setConfig(c: ServerConfig): void {
  cfg = { url: c.url.trim().replace(/\/+$/, ''), token: c.token.trim() }
  localStorage.setItem(STORAGE_KEY, JSON.stringify(cfg))
}

export function getConfig(): ServerConfig {
  return { ...cfg }
}

export class ApiError extends Error {
  status: number
  constructor(status: number, message: string) {
    super(message)
    this.status = status
  }
}

async function request<T>(path: string, init?: RequestInit, timeoutMs = 30 * 60 * 1000): Promise<T> {
  if (!cfg.url) throw new ApiError(0, '未配置服务器地址')
  const headers = new Headers(init?.headers)
  if (cfg.token) headers.set('Authorization', `Bearer ${cfg.token}`)
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), timeoutMs)
  let resp: Response
  try {
    resp = await fetch(cfg.url + path, { ...init, headers, signal: controller.signal })
  } catch (e: any) {
    if (e?.name === 'AbortError') throw new ApiError(0, '请求超时，请检查网络或文件大小')
    throw new ApiError(0, `网络错误：${e?.message || 'Failed to fetch'}`)
  } finally {
    clearTimeout(timer)
  }
  if (!resp.ok) {
    let msg = `HTTP ${resp.status}`
    try {
      const j = await resp.json()
      if (j && j.error) msg = j.error
    } catch {
      /* ignore */
    }
    throw new ApiError(resp.status, msg)
  }
  if (resp.status === 204) return undefined as T
  const text = await resp.text()
  if (!text) return undefined as T
  try {
    return JSON.parse(text) as T
  } catch {
    return text as unknown as T
  }
}

// 带上传进度的 XHR 请求（fetch 不支持上传进度，改用 XMLHttpRequest）
function xhrUpload<T>(
  path: string,
  fd: FormData,
  onProgress?: (loaded: number, total: number) => void,
  timeoutMs = 30 * 60 * 1000,
): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    if (!cfg.url) {
      reject(new ApiError(0, '未配置服务器地址'))
      return
    }
    const xhr = new XMLHttpRequest()
    xhr.open('POST', cfg.url + path)
    if (cfg.token) xhr.setRequestHeader('Authorization', `Bearer ${cfg.token}`)
    xhr.timeout = timeoutMs
    xhr.upload.onprogress = (e) => {
      if (e.lengthComputable && onProgress) onProgress(e.loaded, e.total)
    }
    xhr.onload = () => {
      if (xhr.status >= 200 && xhr.status < 300) {
        const text = xhr.responseText
        if (!text) {
          resolve(undefined as T)
          return
        }
        try {
          resolve(JSON.parse(text) as T)
        } catch {
          resolve(text as unknown as T)
        }
      } else {
        let msg = `HTTP ${xhr.status}`
        try {
          const j = JSON.parse(xhr.responseText)
          if (j && j.error) msg = j.error
        } catch {
          /* ignore */
        }
        reject(new ApiError(xhr.status, msg))
      }
    }
    xhr.onerror = () => reject(new ApiError(0, '网络错误，请检查连接'))
    xhr.ontimeout = () => reject(new ApiError(0, '请求超时，请检查网络或文件大小'))
    xhr.onabort = () => reject(new ApiError(0, '上传已中断'))
    xhr.send(fd)
  })
}

export async function checkServer(url: string, token: string): Promise<boolean> {
  try {
    const resp = await fetch(url.replace(/\/+$/, '') + '/api/health', {
      headers: token ? { Authorization: `Bearer ${token}` } : undefined,
    })
    return resp.ok
  } catch {
    return false
  }
}

export const api = {
  tree: () => request<Tree>('/api/tree'),

  createFolder: (name: string, parent_id: number | null) =>
    request<Folder>('/api/folders', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name, parent_id }),
    }),

  renameFolder: (id: number, name: string) =>
    request<Folder>(`/api/folders/${id}`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name }),
    }),

  moveFolder: (id: number, parent_id: number | null) =>
    request<Folder>(`/api/folders/${id}`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ parent_id: parent_id ?? 0 }),
    }),

  deleteFolder: (id: number) => request<void>(`/api/folders/${id}`, { method: 'DELETE' }),

  getFileDiskPath: (id: number) => request<{ path: string; name: string }>(`/api/files/${id}/disk-path`),

  getFolderDiskPath: (id: number) => request<{ path: string; name: string }>(`/api/folders/${id}/disk-path`),

  upload: (file: File, folder_id: number | null, comment = '', new_file = false) => {
    const fd = new FormData()
    fd.append('file', file)
    fd.append('folder_id', folder_id == null ? '' : String(folder_id))
    fd.append('comment', comment)
    if (new_file) fd.append('new_file', '1')
    return request<{ created: 'file' | 'version'; file: FileMeta; version_no: number }>('/api/files', {
      method: 'POST',
      body: fd,
    })
  },

  addVersion: (id: number, file: File, comment = '') => {
    const fd = new FormData()
    fd.append('file', file)
    fd.append('comment', comment)
    return request<{ version_id: number; version_no: number }>(`/api/files/${id}/versions`, {
      method: 'POST',
      body: fd,
    })
  },

  uploadWithProgress: (
    file: File,
    folder_id: number | null,
    comment = '',
    new_file = false,
    onProgress?: (loaded: number, total: number) => void,
  ) => {
    const fd = new FormData()
    fd.append('file', file)
    fd.append('folder_id', folder_id == null ? '' : String(folder_id))
    fd.append('comment', comment)
    if (new_file) fd.append('new_file', '1')
    return xhrUpload<{ created: 'file' | 'version'; file: FileMeta; version_no: number }>(
      '/api/files',
      fd,
      onProgress,
    )
  },

  addVersionWithProgress: (
    id: number,
    file: File,
    comment = '',
    onProgress?: (loaded: number, total: number) => void,
  ) => {
    const fd = new FormData()
    fd.append('file', file)
    fd.append('comment', comment)
    return xhrUpload<{ version_id: number; version_no: number }>(
      `/api/files/${id}/versions`,
      fd,
      onProgress,
    )
  },

  versions: (id: number) => request<{ file: FileMeta; versions: VersionInfo[] }>(`/api/files/${id}/versions`),

  patchFile: (id: number, patch: { name?: string; folder_id?: number | null; description?: string }) =>
    request<FileMeta>(`/api/files/${id}`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(patch),
    }),

  deleteFile: (id: number) => request<void>(`/api/files/${id}`, { method: 'DELETE' }),

  search: (q: string) => request<{ files: FileMeta[] }>(`/api/search?q=${encodeURIComponent(q)}`),

  fetchBlob: async (path: string): Promise<Blob> => {
    if (!cfg.url) throw new ApiError(0, '未配置服务器地址')
    const headers = new Headers()
    if (cfg.token) headers.set('Authorization', `Bearer ${cfg.token}`)
    const resp = await fetch(cfg.url + path, { headers })
    if (!resp.ok) {
      let msg = `HTTP ${resp.status}`
      try {
        const j = await resp.json()
        if (j && j.error) msg = j.error
      } catch {
        /* ignore */
      }
      throw new ApiError(resp.status, msg)
    }
    return resp.blob()
  },

  request: <T>(path: string, init?: RequestInit) => request<T>(path, init),

  downloadUrl: (id: number, version?: number) => `/api/files/${id}/download${version ? `?version=${version}` : ''}`,
  previewUrl: (id: number, version?: number) => `/api/files/${id}/preview${version ? `?version=${version}` : ''}`,
  dxfUrl: (id: number, version?: number) => `/api/files/${id}/dxf${version ? `?version=${version}` : ''}`,
  archiveListUrl: (id: number, version?: number) => `/api/files/${id}/archive-list${version ? `?version=${version}` : ''}`,
  archiveEntryUrl: (id: number, path: string, version?: number) =>
    `/api/files/${id}/archive-entry?path=${encodeURIComponent(path)}${version ? `&version=${version}` : ''}`,
}
