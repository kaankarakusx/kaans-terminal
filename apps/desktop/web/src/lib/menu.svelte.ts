// Tek bir sağ tık / açılır menü. Dışarı tıklayınca ya da Esc ile kapanır.

export interface MenuItem {
  label: string
  action: () => void
  checked?: boolean
  disabled?: boolean
  danger?: boolean
}

class Menu {
  open = $state<{ x: number; y: number; items: (MenuItem | 'sep')[] } | null>(null)

  show(x: number, y: number, items: (MenuItem | 'sep')[]) {
    this.open = { x, y, items }
  }

  close() {
    this.open = null
  }
}

export const menu = new Menu()
