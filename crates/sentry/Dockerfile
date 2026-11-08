# aster-sentry — tiered runtime security control. L1 (rules) + L2 (LLM) work out of the box; L3 (the
# aster agent) additionally needs Node + @aster-ai-lab/code — layer those into a derived image.
FROM rust:1-slim AS build
WORKDIR /src
COPY . .
RUN cargo build --release --bin sentry

FROM debian:stable-slim
# -slim ships a shell + coreutils (tail), which the `collector | sentry` / `tail -F | sentry` deploy
# patterns use. Drop privileges: sentry needs no root — it only appends to the deny-files it's given.
RUN useradd --system --uid 10001 sentry
COPY --from=build /src/target/release/sentry /usr/local/bin/aster-sentry
# L3 reference assets (the agent bridge + skill playbooks); inert for L1/L2-only deployments.
COPY scripts/l3-agent.mjs /opt/sentry/l3-agent.mjs
COPY skills/ /opt/sentry/skills/
USER sentry
ENTRYPOINT ["aster-sentry"]
