import { describe, expect, it } from 'vitest'
import { richText } from './richText'

describe('richText', () => {
  it('links urls and @github names without touching emails', () => {
    expect(richText('see https://x.dev/a?b=1, thanks @kaankarakusx! mail me a@b.com')).toEqual([
      { text: 'see ' },
      { text: 'https://x.dev/a?b=1', href: 'https://x.dev/a?b=1' },
      { text: ', thanks ' },
      { text: '@kaankarakusx', href: 'https://github.com/kaankarakusx' },
      { text: '! mail me a@b.com' },
    ])
    expect(richText('@ada starts')).toEqual([{ text: '@ada', href: 'https://github.com/ada' }, { text: ' starts' }])
    expect(richText('plain')).toEqual([{ text: 'plain' }])
  })
})
