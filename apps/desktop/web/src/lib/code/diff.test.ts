import { describe, expect, it } from 'vitest'
import { allAdded, lineCount, parsePatch, stickyLines } from './diff'

describe('parsePatch', () => {
  it('numbers context, added and deleted lines per hunk', () => {
    const patch = [
      'diff --git a/x b/x',
      'index 1..2 100644',
      '--- a/x',
      '+++ b/x',
      '@@ -1,3 +1,3 @@ fn main() {',
      ' a',
      '-b',
      '+B',
      ' c',
      '@@ -10,2 +10,3 @@',
      ' j',
      '+k',
      ' l',
      '\\ No newline at end of file',
    ].join('\n')
    const d = parsePatch(patch)
    expect(d[0]).toEqual({ kind: 'hunk', old: null, new: null, text: 'fn main() {' })
    expect(d.slice(1, 5).map((l) => [l.kind, l.old, l.new])).toEqual([
      ['ctx', 1, 1],
      ['del', 2, null],
      ['add', null, 2],
      ['ctx', 3, 3],
    ])
    expect(d.slice(6).map((l) => [l.kind, l.old, l.new])).toEqual([
      ['ctx', 10, 10],
      ['add', null, 11],
      ['ctx', 11, 12],
    ])
  })

  it('treats new files as all added', () => {
    expect(lineCount('a\nb\n')).toBe(2)
    expect(lineCount('a\nb')).toBe(2)
    expect(allAdded(2).map((l) => l.new)).toEqual([null, 1, 2])
  })
})

describe('stickyLines', () => {
  it('finds the enclosing block headers', () => {
    const src = ['impl Foo {', '    fn bar() {', '        let x = 1;', '', '        x', '    }', '}'].join('\n').split('\n')
    expect(stickyLines(src, 4)).toEqual([0, 1])
    expect(stickyLines(src, 6)).toEqual([])
  })
})
