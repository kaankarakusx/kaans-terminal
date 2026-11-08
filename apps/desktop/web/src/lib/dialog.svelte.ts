// Uygulama içi onay penceresi (tarayıcının bloklayan confirm'ü yerine).

export interface ConfirmRequest {
  title: string
  body: string
  action: string
  danger?: boolean
  run: () => void | Promise<void>
}

class Confirm {
  open = $state<ConfirmRequest | null>(null)

  ask(req: ConfirmRequest) {
    this.open = req
  }

  async accept() {
    const r = this.open
    this.open = null
    await r?.run()
  }

  cancel() {
    this.open = null
  }
}

export const confirm = new Confirm()
