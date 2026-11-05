import { useSyncExternalStore } from 'react'

// The page's theme: the system's, unless the footer switch set data-theme on <html>.
const media = () => window.matchMedia('(prefers-color-scheme: dark)')

function subscribe(change: () => void) {
  const m = media()
  m.addEventListener('change', change)
  const observer = new MutationObserver(change)
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] })
  return () => {
    m.removeEventListener('change', change)
    observer.disconnect()
  }
}

const isDark = () => {
  const set = document.documentElement.dataset.theme
  return set ? set === 'dark' : media().matches
}

export const useDark = () => useSyncExternalStore(subscribe, isDark, () => false)
