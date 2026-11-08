#!/usr/bin/env bash
# Yerelden çalışır: server/ + packages/shared'i tar ile sunucuya yükler, imajı yeniden kurar, sağlık kontrolü yapar.
#   HOST=root@<sunucu> KEY=~/.ssh/<anahtar> ./server/deploy/deploy.sh      (repo kökünden; değerler yereldeki DEPLOYMENT.md'de)
# Sunucudaki .env'e ve veritabanı birimine dokunmaz.
set -euo pipefail
HOST="${HOST:?HOST=root@<sunucu> ver}"
KEY="${KEY:?KEY=~/.ssh/<anahtar> ver}"
DIR=/root/kaans-terminal
cd "$(git rev-parse --show-toplevel)"

COPYFILE_DISABLE=1 tar czf - --no-xattrs --exclude node_modules --exclude data --exclude '*.test.ts' \
  server/package.json server/package-lock.json server/Dockerfile server/src server/scripts \
  server/deploy/docker-compose.yml server/deploy/nginx.conf packages/shared \
  | ssh -i "$KEY" "$HOST" "set -e; mkdir -p $DIR && cd $DIR && rm -rf server/src server/scripts packages/shared && tar xzf - --no-same-owner"

ssh -i "$KEY" "$HOST" "set -e; cd $DIR/server/deploy && docker compose up -d --build && docker image prune -f >/dev/null
for i in \$(seq 1 20); do
  if curl -fsS -o /dev/null http://127.0.0.1:4000/api/v1/health; then echo 'deploy OK'; exit 0; fi
  sleep 2
done
echo 'sağlık kontrolü BAŞARISIZ' >&2; docker compose logs --tail 40 app; exit 1"
