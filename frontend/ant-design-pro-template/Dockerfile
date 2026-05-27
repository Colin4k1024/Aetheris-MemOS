# Build stage
FROM node:20-alpine AS builder

WORKDIR /app

# Use China npm mirror to avoid EIDLETIMEOUT in Docker builds
RUN npm config set registry https://registry.npmmirror.com

COPY package.json package-lock.json* ./
RUN npm install --legacy-peer-deps --fetch-timeout=120000

COPY . .
RUN npm run build

# Runtime stage
FROM nginx:alpine

COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]
