# ─────────────────────────────────────────────────────────────────────────────
# PrxyClaude · Dockerfile
# ─────────────────────────────────────────────────────────────────────────────

FROM node:20-alpine AS base
WORKDIR /app

# ── Dependencies ──────────────────────────────────────────────────────────────
FROM base AS deps
COPY package*.json ./
RUN npm ci --only=production --ignore-scripts

# ── Build ─────────────────────────────────────────────────────────────────────
FROM base AS builder
COPY package*.json tsconfig.json ./
RUN npm ci
COPY src/ ./src/
RUN npm run build

# ── Runtime ───────────────────────────────────────────────────────────────────
FROM base AS runtime
ENV NODE_ENV=production

# Copy production deps + compiled output
COPY --from=deps /app/node_modules ./node_modules
COPY --from=builder /app/dist ./dist
COPY package.json ./

# Non-root user
RUN addgroup -g 1001 -S prxy && adduser -S prxy -G prxy -u 1001
USER prxy

EXPOSE 8082

HEALTHCHECK --interval=15s --timeout=5s --start-period=5s --retries=3 \
  CMD wget -qO- http://localhost:8082/health || exit 1

CMD ["node", "dist/server.js"]
