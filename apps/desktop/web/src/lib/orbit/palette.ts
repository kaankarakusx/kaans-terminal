// orbit.map kendi renklerini kullanır; temadan bağımsızdır (TASARIM.md §6).
import type { Category, Status } from '../types'

export const SPACE = '#040406'
export const CORE = '#ffb057'
export const CORE_HOT = '#f88008'

export const STATUS: Record<Status, string> = {
  busy: '#f88008',
  idle: '#00e5ff',
  waiting: '#ff2e63',
  ended: '#5c4d40',
}

export const SUBAGENT = '#9d6bff'

/** --hue0 … --hue9 ile aynı sıra */
export const HUES = [
  '#ffb057', '#00e5ff', '#9d6bff', '#3dffa8', '#ff5c8a',
  '#5b8cff', '#ffd23f', '#b8ff3d', '#e879f9', '#f5ebe0',
]

export const CATEGORY: Record<Category, string> = {
  web: '#38bdf8',
  browser: '#5b8cff',
  read: '#3dffa8',
  search: '#b8ff3d',
  edit: '#ffd23f',
  bash: '#f88008',
  agent: '#9d6bff',
  mcp: '#e879f9',
  plan: '#b39a82',
  other: '#8f7a66',
}

export const ERROR = '#ff4d4d'
