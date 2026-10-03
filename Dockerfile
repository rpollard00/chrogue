# The image serves the web build (ports/web). ports/web/build.sh makes the files on the workstation: the build needs
# the Emscripten SDK and Rust, and this image has only the server.
FROM busybox:1.37.0
COPY httpd.conf /etc/httpd.conf
COPY ports/web/dist/ /www/
USER 65534:65534
EXPOSE 80
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 CMD ["wget", "-q", "-O", "/dev/null", "http://127.0.0.1/"]
ENTRYPOINT ["httpd", "-f", "-p", "80", "-h", "/www", "-c", "/etc/httpd.conf"]
