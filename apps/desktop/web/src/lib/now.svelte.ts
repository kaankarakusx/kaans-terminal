// Saniyede bir güncellenen "şimdi"; sayfa gizliyken durur.
class Now {
  value = $state(Date.now())
  constructor() {
    if (typeof window === 'undefined') return
    setInterval(() => {
      if (!document.hidden) this.value = Date.now()
    }, 1000)
  }
}
export const now = new Now()
