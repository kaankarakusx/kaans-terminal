import { describe, expect, it } from 'vitest'
import { layoutGraph } from './gitGraph'
import type { GitCommit } from './types'

const c = (hash: string, parents: string[] = []): GitCommit => ({ hash, parents, author: 'a', time: 0, refs: [], subject: hash })

describe('layoutGraph', () => {
  it('keeps a linear history in one lane', () => {
    const { rows, width } = layoutGraph([c('c', ['b']), c('b', ['a']), c('a')])
    expect(rows.map((r) => r.col)).toEqual([0, 0, 0])
    expect(width).toBe(1)
    expect(rows[2].below).toEqual([])
  })

  it('opens a lane for a merge parent and closes it where branches meet', () => {
    // m = merge(x, y); x -> base; y -> base
    const { rows, width } = layoutGraph([c('m', ['x', 'y']), c('x', ['base']), c('y', ['base']), c('base')])
    expect(width).toBe(2)
    expect(rows[0].below).toEqual(['x', 'y'])
    expect(rows[1].col).toBe(0)
    expect(rows[2].col).toBe(1)
    // y'nin ebeveyni base zaten 0. şeritte bekleniyor; iki şerit base'de birleşir
    expect(rows[2].below).toEqual(['base', 'base'])
    expect(rows[3].col).toBe(0)
    expect(rows[3].below).toEqual([])
  })

  it('puts an unrelated tip (e.g. upstream ahead) in its own lane', () => {
    const { rows } = layoutGraph([c('u2', ['u1']), c('h1', ['base']), c('u1', ['base']), c('base')])
    expect(rows[0].col).toBe(0)
    expect(rows[1].col).toBe(1)
    expect(rows[2].col).toBe(0)
    expect(rows[3].col).toBe(0)
  })
})
