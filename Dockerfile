FROM oven/bun:1.4.2 AS build
WORKDIR /build
COPY package.json bun.lock ./
RUN bun install --frozen-lockfile
COPY index.html style.css vite.config.ts tsconfig.json ./
COPY src/ src/
RUN bun run build

FROM busybox:1.37.0
COPY httpd.conf /etc/httpd.conf
COPY --from=build /build/dist/ /www/
USER 65534:65534
EXPOSE 80
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 CMD ["wget", "-q", "-O", "/dev/null", "http://127.0.0.1/"]
ENTRYPOINT ["httpd", "-f", "-p", "80", "-h", "/www", "-c", "/etc/httpd.conf"]
