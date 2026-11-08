// Veritabanı katmanı: üretimde Postgres (`pg`), testlerde ve yerel geliştirmede PGlite (süreç içi gerçek Postgres).
// Uygulama yalnızca bu küçük arayüzü kullanır.
import pg from 'pg'

export interface Db {
  query<T = Record<string, unknown>>(sql: string, params?: unknown[]): Promise<T[]>
  /** Tek bağlantıda işlem (BEGIN/COMMIT); hata olursa ROLLBACK */
  tx<T>(fn: (q: Db['query']) => Promise<T>): Promise<T>
  close(): Promise<void>
}

export function postgres(url: string): Db {
  const pool = new pg.Pool({ connectionString: url, max: 10 })
  return {
    async query(sql, params) {
      return (await pool.query(sql, params as unknown[])).rows
    },
    async tx(fn) {
      const c = await pool.connect()
      try {
        await c.query('BEGIN')
        const r = await fn(async (sql, params) => (await c.query(sql, params as unknown[])).rows)
        await c.query('COMMIT')
        return r
      } catch (e) {
        await c.query('ROLLBACK')
        throw e
      } finally {
        c.release()
      }
    },
    close: () => pool.end(),
  }
}

/** PGlite: `dir` yoksa bellekte (testler). Yalnızca geliştirme bağımlılığı. */
export async function pglite(dir?: string): Promise<Db> {
  const { PGlite } = await import('@electric-sql/pglite')
  if (dir) (await import('node:fs')).mkdirSync(dir, { recursive: true })
  const db = new PGlite(dir)
  // PGlite tek bağlantılıdır: işlemler sıraya girer
  let chain: Promise<unknown> = Promise.resolve()
  const query: Db['query'] = async (sql, params) => (await db.query(sql, params as unknown[])).rows as never
  return {
    query,
    tx(fn) {
      const run = chain.then(() => db.transaction((t) => fn(async (sql, params) => (await t.query(sql, params as unknown[])).rows as never)))
      chain = run.catch(() => {})
      return run
    },
    close: () => db.close(),
  }
}

/** Şema (MIMARI.md §6.2). Sürüm arttıkça yeni adımlar eklenir; var olan adım değiştirilmez. */
const MIGRATIONS: string[] = [
  `
  create table devices (
    id bigserial primary key,
    device_hash text not null unique,
    token_hash text not null unique,
    hacker_name text not null,
    name_key text not null,
    tag text not null,
    os text,
    app_version text,
    created_at timestamptz not null default now(),
    last_seen timestamptz,
    unique (name_key, tag)
  );
  create table profiles (
    device_id bigint primary key references devices(id) on delete cascade,
    theme text,
    tz_offset integer not null default 0,
    tz_changed_day text,
    last_hb timestamptz,
    last_active timestamptz,
    hide_time boolean not null default false,
    hide_name boolean not null default false,
    xp_total integer not null default 0,
    level integer not null default 1,
    week_start text,
    week_xp integer not null default 0
  );
  create table activity_days (
    device_id bigint not null references devices(id) on delete cascade,
    day text not null,
    active_seconds integer not null default 0,
    primary key (device_id, day)
  );
  `,
  // 2: arkadaşlar, mesajlar, durum (online/away/invisible)
  `
  alter table profiles add column presence text not null default 'online';
  create table friendships (
    a bigint not null references devices(id) on delete cascade,
    b bigint not null references devices(id) on delete cascade,
    status text not null,
    requested_by bigint not null,
    created_at timestamptz not null default now(),
    primary key (a, b),
    check (a < b)
  );
  create index friendships_b on friendships (b);
  create table messages (
    id bigserial primary key,
    from_id bigint not null references devices(id) on delete cascade,
    to_id bigint not null references devices(id) on delete cascade,
    body text not null,
    created_at timestamptz not null default now(),
    read_at timestamptz
  );
  create index messages_pair on messages (from_id, to_id, id);
  create index messages_unread on messages (to_id) where read_at is null;
  `,
  // 3: duyurular ve geri bildirim
  `
  create table announcements (
    id bigserial primary key,
    title text not null,
    body text not null,
    created_at timestamptz not null default now()
  );
  create table feedback (
    id bigserial primary key,
    device_id bigint references devices(id) on delete set null,
    name_tag text not null,
    body text not null,
    app_version text,
    created_at timestamptz not null default now()
  );
  `,
  // 4: uçtan uca şifreli mesajlar. Sunucu yalnızca açık anahtarları ve şifreli metni görür; body artık şifreli
  // metin (hex). Şifreleme öncesi düz mesajlar silinir (yalnızca geliştirme sürümlerinde vardı).
  `
  alter table devices add column public_key text;
  delete from messages;
  alter table messages add column nonce text not null default '';
  alter table messages add column from_key text not null default '';
  alter table messages add column to_key text not null default '';
  `,
]

export async function migrate(db: Db): Promise<void> {
  await db.query('create table if not exists schema_version (version integer not null)')
  const rows = await db.query<{ version: number }>('select version from schema_version')
  let v = rows[0]?.version ?? 0
  if (!rows.length) await db.query('insert into schema_version (version) values (0)')
  for (; v < MIGRATIONS.length; v++) {
    await db.tx(async (q) => {
      for (const stmt of MIGRATIONS[v].split(';').map((s) => s.trim()).filter(Boolean)) await q(stmt)
      await q('update schema_version set version = $1', [v + 1])
    })
  }
}
