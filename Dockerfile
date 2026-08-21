FROM node:24-alpine AS build
WORKDIR /app
COPY apps/web/package.json apps/web/package-lock.json ./
RUN npm ci
COPY apps/web/ ./
ARG VITE_CHAIN_ID
ARG VITE_CHAIN_NAME
ARG VITE_RPC_URL
ARG VITE_CLAIM_ADDRESS
ARG VITE_EXPLORER_NAME
ARG VITE_EXPLORER_URL
RUN npm run build

FROM caddy:2-alpine
COPY --from=build /app/dist /srv
CMD ["sh", "-c", "caddy file-server --root /srv --listen :${PORT:-3000}"]
