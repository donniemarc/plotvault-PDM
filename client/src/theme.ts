import { ref, type Ref } from 'vue'
import { isTauri } from './utils'

export type ThemeMode = 'system' | 'light' | 'dark'
export type ResolvedTheme = 'light' | 'dark'
export const THEME_STORAGE_KEY = 'plotvault_pdm_theme'

export const themeMode: Ref<ThemeMode> = ref(getStoredMode())

let currentResolved: ResolvedTheme = 'dark'
const listeners = new Set<(theme: ResolvedTheme) => void>()

function getStoredMode(): ThemeMode {
  try {
    const v = localStorage.getItem(THEME_STORAGE_KEY)
    if (v === 'light' || v === 'dark' || v === 'system') return v
  } catch {
    /* ignore */
  }
  return 'system'
}

export function resolveTheme(mode: ThemeMode): ResolvedTheme {
  if (mode === 'light' || mode === 'dark') return mode
  try {
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
  } catch {
    return 'dark'
  }
}

export function getResolvedTheme(): ResolvedTheme {
  return currentResolved
}

export function getViewerBg(): string {
  try {
    const v = getComputedStyle(document.documentElement)
      .getPropertyValue('--viewer-bg')
      .trim()
    if (v) return v
  } catch {
    /* ignore */
  }
  return '#0f1419'
}

function notify(theme: ResolvedTheme) {
  listeners.forEach((cb) => {
    try {
      cb(theme)
    } catch {
      /* ignore */
    }
  })
}

export function onThemeChanged(cb: (theme: ResolvedTheme) => void): () => void {
  listeners.add(cb)
  return () => {
    listeners.delete(cb)
  }
}

async function syncTauri(mode: ThemeMode) {
  if (!isTauri()) return
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    await getCurrentWindow().setTheme(mode === 'system' ? null : mode)
  } catch {
    /* 静默降级：仅原生标题栏不联动 */
  }
}

export function applyTheme(mode: ThemeMode): ResolvedTheme {
  const resolved = resolveTheme(mode)
  currentResolved = resolved
  document.documentElement.setAttribute('data-theme', resolved)
  try {
    localStorage.setItem(THEME_STORAGE_KEY, mode)
  } catch {
    /* ignore */
  }
  notify(resolved)
  void syncTauri(mode)
  return resolved
}

export function setThemeMode(mode: ThemeMode) {
  themeMode.value = mode
  applyTheme(mode)
}

export function initTheme() {
  applyTheme(themeMode.value)
  try {
    const mql = window.matchMedia('(prefers-color-scheme: dark)')
    const handler = () => {
      if (themeMode.value === 'system') applyTheme('system')
    }
    if (typeof mql.addEventListener === 'function') mql.addEventListener('change', handler)
    else (mql as any).addListener(handler)
  } catch {
    /* ignore */
  }
}