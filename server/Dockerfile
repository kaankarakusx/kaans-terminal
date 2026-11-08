# Kaan's Terminal sosyal sunucusu. Yapı bağlamı repo kökü (packages/shared gerekir):
#   docker build -f server/Dockerfile .
# Node ≥22.18 TypeScript'i doğrudan çalıştırır (derleme yok). Sürümler sabit.
FROM node:22.23.3-alpine3.24
WORKDIR /app/server
COPY packages/shared /app/packages/shared
COPY server/package.json server/package-lock.json ./
RUN npm ci --omit=dev --no-audit --no-fund && npm cache clean --force
COPY server/src ./src
COPY server/scripts ./scripts
ENV NODE_ENV=production HOST=0.0.0.0 PORT=8787 TRUST_PROXY=1
USER node
EXPOSE 8787
CMD ["node", "src/main.ts"]
