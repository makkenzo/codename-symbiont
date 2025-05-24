FROM node:22-alpine AS deps

WORKDIR /app

COPY package.json pnpm-lock.yaml ./

RUN npm install -g pnpm && pnpm install --frozen-lockfile

COPY . .

ARG NEXT_PUBLIC_API_URL 
ENV NEXT_PUBLIC_API_URL=${NEXT_PUBLIC_API_URL}

RUN pnpm build

FROM node:22-alpine AS runner 

WORKDIR /app

ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED 1

COPY --from=deps /app/public ./public
COPY --from=deps /app/.next/standalone ./ 

COPY --from=deps /app/.next/static ./.next/static

EXPOSE 3000

CMD ["node", "server.js"]