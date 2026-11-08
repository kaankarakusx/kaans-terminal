// Panel kimliği → bileşen. Yeni panel: layout.svelte.ts'deki PanelId'e ve buraya ekle.
import type { Component } from 'svelte'
import type { PanelId } from './layout.svelte'
import FilesHeat from './panels/FilesHeat.svelte'
import ResearchFeed from './panels/ResearchFeed.svelte'
import SpendLedger from './panels/SpendLedger.svelte'
import ToolMix from './panels/ToolMix.svelte'
import Workers from './panels/Workers.svelte'
import SessionList from './SessionList.svelte'
import SessionLog from './SessionLog.svelte'
import YourCall from './YourCall.svelte'

export const PANELS: Record<PanelId, Component> = {
  sessions: SessionList,
  'research.feed': ResearchFeed,
  'files.heat': FilesHeat,
  'tool.mix': ToolMix,
  'session.log': SessionLog,
  workers: Workers,
  'spend.ledger': SpendLedger,
  'your.call': YourCall,
}
