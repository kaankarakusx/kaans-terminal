import { describe, expect, it } from 'vitest'
import {
  RANKS, dayBaseXp, heartbeatCredit, levelOf, rankOf, restedLeftMin, streakBonus, summarize, totalXp, weekOf, xpToNext,
} from './xp'

describe('day XP', () => {
  it('doubles the first 2 hours, halves after 8 hours, adds the daily bonus', () => {
    expect(dayBaseXp(0)).toBe(0)
    expect(dayBaseXp(59)).toBe(0)
    expect(dayBaseXp(60)).toBe(50 + 2)
    expect(dayBaseXp(120 * 60)).toBe(50 + 240)
    expect(dayBaseXp(300 * 60)).toBe(50 + 240 + 180)
    expect(dayBaseXp(600 * 60)).toBe(50 + 240 + 360 + 60)
    expect(restedLeftMin(30 * 60)).toBe(90)
    expect(restedLeftMin(200 * 60)).toBe(0)
  })
})

describe('streak', () => {
  it('weeks start on monday', () => {
    expect(weekOf('2026-11-08')).toBe('2026-11-02') // pazar
    expect(weekOf('2026-11-02')).toBe('2026-11-02')
  })

  it('adds 10% per consecutive qualifying week, up to 50%', () => {
    expect(streakBonus(0)).toBe(0)
    expect(streakBonus(3)).toBeCloseTo(0.3)
    expect(streakBonus(9)).toBe(0.5)
    const day = (d: string, min: number) => ({ day: d, activeSecs: min * 60 })
    // 26 Ekim haftası şartı sağlar (3 gün ≥30 dk); 2 Kasım haftası çarpan alır
    const days = [day('2026-10-26', 30), day('2026-10-27', 40), day('2026-10-28', 31), day('2026-11-03', 60)]
    const base = dayBaseXp(30 * 60) + dayBaseXp(40 * 60) + dayBaseXp(31 * 60)
    const r = totalXp(days, '2026-11-08')
    expect(r.streakWeeks).toBe(1)
    expect(r.total).toBe(Math.floor(base + dayBaseXp(60 * 60) * 1.1))
    // iki gün yetmez
    expect(totalXp([day('2026-10-26', 300), day('2026-10-27', 300), day('2026-11-03', 10)], '2026-11-08').streakWeeks).toBe(0)
  })
})

describe('levels and ranks', () => {
  it('follows 2.2 × (L² + 9L) rounded to 10', () => {
    expect(xpToNext(1)).toBe(20)
    expect(xpToNext(10)).toBe(420)
    expect(xpToNext(60)).toBe(0)
    expect(levelOf(0)).toEqual({ level: 1, levelXp: 0, nextXp: 20 })
    expect(levelOf(19).level).toBe(1)
    expect(levelOf(20)).toEqual({ level: 2, levelXp: 0, nextXp: xpToNext(2) })
    expect(levelOf(1e9).level).toBe(60)
  })

  it('maps levels to the seven guild ranks', () => {
    expect(RANKS).toHaveLength(7)
    expect([1, 4, 5, 14, 15, 25, 35, 45, 54, 55, 60].map((l) => rankOf(l).id)).toEqual([
      'apprentice', 'apprentice', 'adept', 'adept', 'conjurer', 'runesmith', 'archmage', 'oracle', 'oracle', 'eternal', 'eternal',
    ])
    expect(summarize([], '2026-11-08').rank.name).toBe('Apprentice')
  })

  it('credits heartbeats like the server', () => {
    expect(heartbeatCredit(60)).toBe(60)
    expect(heartbeatCredit(120)).toBe(90)
    expect(heartbeatCredit(400)).toBe(60)
  })
})
