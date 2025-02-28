# syntax=docker/dockerfile:1
FROM rust:1.82 AS builder
WORKDIR /usr/src/ldml-api
COPY . .
RUN cargo install --path .

FROM debian:bookworm-slim
ENV LDML_DEFAULT_PROFILE=production
ENV LDML_CONFIG=/var/lib/ldml-api/config.json
RUN --mount=type=cache,target=/var/cache/apt \
    --mount=type=cache,target=/var/lib/apt \
<<EOT
    apt-get update
    apt-get -y install libxml2
EOT
COPY --from=builder /usr/local/cargo/bin/ldml-api /usr/local/bin/
COPY <<EOF /var/lib/ldml-api/config.json
{
    "staging": {
        "langtags": "/var/lib/ldml-api/langtags/staging",
        "sldr": "/var/lib/ldml-api/sldr/staging"
    },
    "production": {
        "langtags": "/var/lib/ldml-api/langtags/production",
        "sldr": "/var/lib/ldml-api/sldr/production"
    }
}
EOF
VOLUME /var/lib/ldml-api/sldr
VOLUME /var/lib/ldml-api/langtags
CMD ["sh", "-c", "exec ldml-api --config=${LDML_CONFIG} --profile=${LDML_DEFAULT_PROFILE}"]
