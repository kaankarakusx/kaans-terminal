// Sesler Web Audio ile sentezlenir; ses dosyası yoktur (TASARIM.md §8).

let ctx: AudioContext | null = null

function audio(): AudioContext | null {
  if (typeof AudioContext === 'undefined') return null
  ctx ??= new AudioContext()
  // Tarayıcılar ilk kullanıcı hareketinden önce sesi askıya alır; ses açma tıklaması bunu çözer.
  if (ctx.state === 'suspended') void ctx.resume()
  return ctx
}

/** Ses açılınca çağrılır (kullanıcı hareketi içinde): bağlamı hazırlar. */
export function unlock() {
  audio()
}

function note(freq: number, start: number, dur: number, type: OscillatorType = 'sine', gain = 0.12) {
  const a = audio()
  if (!a) return
  const t = a.currentTime + start
  const osc = a.createOscillator()
  const g = a.createGain()
  osc.type = type
  osc.frequency.setValueAtTime(freq, t)
  g.gain.setValueAtTime(0, t)
  g.gain.linearRampToValueAtTime(gain, t + 0.012)
  g.gain.exponentialRampToValueAtTime(0.0001, t + dur)
  osc.connect(g).connect(a.destination)
  osc.start(t)
  osc.stop(t + dur + 0.02)
}

/** Bir oturum onay ya da soru bekliyor: iki tonlu kısa uyarı. */
export function attention() {
  note(880, 0, 0.16, 'triangle')
  note(1318.5, 0.13, 0.22, 'triangle')
}

/** Same file: alçak, iki vuruşlu uyarı. */
export function warning() {
  note(220, 0, 0.18, 'square', 0.06)
  note(196, 0.2, 0.24, 'square', 0.06)
}

/** DANGER: kaçak süreç alarmı (iki tonlu korna). */
export function alarm() {
  for (let i = 0; i < 3; i++) {
    note(440, i * 0.36, 0.17, 'sawtooth', 0.07)
    note(554, i * 0.36 + 0.18, 0.17, 'sawtooth', 0.07)
  }
}

/** Level atlama: kısa yükselen arp (rün halkasıyla birlikte). */
export function levelChime() {
  ;[523.25, 659.25, 783.99, 1046.5].forEach((f, i) => note(f, i * 0.07, 0.35, 'triangle', 0.1))
}

/** Rütbe atlama: daha uzun, iki katmanlı fanfar. */
export function fanfare() {
  const seq = [392, 523.25, 659.25, 783.99, 1046.5, 1318.5]
  seq.forEach((f, i) => note(f, i * 0.09, 0.5, 'triangle', 0.1))
  ;[523.25, 659.25, 783.99].forEach((f) => note(f, 0.6, 1.4, 'sine', 0.07))
  ;[1046.5, 1318.5, 1568].forEach((f) => note(f, 0.62, 1.2, 'sine', 0.04))
}
