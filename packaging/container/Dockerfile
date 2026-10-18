# Phygital Key gate as a container (ghcr.io/phygital-key/phygital-key).
# Built by .github/workflows/release.yml from the same Linux binaries the signed
# release contains (their SHA-256 is in that release's signed release.txt).
#
#   docker run -d --name phygital-key -p 127.0.0.1:7468:7468 \
#     -v phygital-key:/data ghcr.io/phygital-key/phygital-key:latest
#   docker exec -it phygital-key phygital-key enroll
#   docker exec -it phygital-key phygital-key mcp-token
#
# Publish the port on 127.0.0.1 only, or put a TLS proxy in front and pass
# --origin https://your.host. See docs/own-server.md.
FROM gcr.io/distroless/cc-debian12:nonroot@sha256:9dac0a79194e45a7da0158a9c6da57b217585af0786db3845d1f0ec1a0dd182f
ARG TARGETARCH
LABEL org.opencontainers.image.title="Phygital Key" \
      org.opencontainers.image.description="A person approves exact AI actions with a passkey; any service checks the signed yes offline." \
      org.opencontainers.image.source="https://github.com/Phygital-Key/phygital-key-mcp" \
      org.opencontainers.image.licenses="Apache-2.0"
COPY --chmod=0755 bin/${TARGETARCH}/phygital-key /usr/local/bin/phygital-key
COPY --chown=65532:65532 data /data
COPY NOTICE LICENSE /usr/share/doc/phygital-key/
USER 65532:65532
ENV PHYGITAL_KEY_HOME=/data
VOLUME ["/data"]
EXPOSE 7468
ENTRYPOINT ["/usr/local/bin/phygital-key"]
# Inside the container the gate must listen on all addresses so Docker can reach it.
CMD ["--bind", "0.0.0.0", "serve"]
