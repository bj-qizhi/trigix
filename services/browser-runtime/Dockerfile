FROM node:22-bookworm-slim AS build
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY tsconfig.json eslint.config.js ./
COPY src ./src
RUN npm run build && npm prune --omit=dev

FROM mcr.microsoft.com/playwright:v1.55.1-noble
ENV NODE_ENV=production \
    BROWSER_RUNTIME_HOST=0.0.0.0 \
    BROWSER_RUNTIME_PORT=38100 \
    BROWSER_ARTIFACT_DIR=/data/browser
WORKDIR /app
RUN groupadd --system --gid 10001 trigix && useradd --system --uid 10001 --gid trigix --create-home trigix \
    && mkdir -p /data/browser && chown -R trigix:trigix /data/browser /app
COPY --from=build --chown=trigix:trigix /app/package.json /app/package-lock.json ./
COPY --from=build --chown=trigix:trigix /app/node_modules ./node_modules
COPY --from=build --chown=trigix:trigix /app/dist/src ./dist
USER trigix
EXPOSE 38100
HEALTHCHECK --interval=15s --timeout=5s --start-period=30s --retries=3 CMD node -e "fetch('http://127.0.0.1:38100/health').then(r=>{if(!r.ok)process.exit(1)}).catch(()=>process.exit(1))"
CMD ["node", "dist/main.js"]
