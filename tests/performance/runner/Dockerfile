# P0.1 step 7/8: the pinned build+measure environment for
# tests/performance's container pair. The mock and ssh workloads are both
# measured in here (never on the contributor's host), so a run today and a
# run next month share an OS, libc, toolchain and set of system packages —
# see tests/performance/README.md.
#
# `rust-toolchain.toml` pins `channel = "stable"`, which is not a fixed
# compiler: this image resolves it once at build time (never hardcoded
# here) and bakes the result in. Reproducible within one build of this
# image, loudly non-comparable across two: `runner.runner_image` (this
# image's digest) changes whenever it is rebuilt, and `runner.toolchain`
# (the resolved `rustc --version`) records exactly what it resolved to.
FROM debian:bookworm-slim@sha256:abd67ffcfa541b485a3dff59865ab629aa048a6c613e639d36e7456b0b229241

# Emulation must never be entered by accident: `--platform` on `build`/`run`
# picks the variant, and this guard fails loudly if that ever resolves to
# something unexpected instead of silently running under emulation.
RUN arch="$(uname -m)" \
	&& case "$arch" in \
	aarch64 | x86_64) ;; \
	*) \
		echo "unsupported arch: $arch" >&2; \
		exit 1 \
		;; \
	esac

# bash: the scripts under scripts/ require it. jq: report/validator
# tooling. GNU coreutils (already Debian's default `date`/`time` provider)
# gives `date +%s%N` and `time -v`. gcc/g++/cmake/make/pkg-config/perl:
# building aws-lc-sys (rustls's default crypto provider) from source.
# openssh-client: `ssh-keyscan` for the fixture-readiness wait. git: cargo
# needs it for git dependencies.
RUN apt-get update && apt-get install -y --no-install-recommends \
	bash ca-certificates cmake curl gcc g++ git jq libc6-dev make \
	openssh-client perl pkg-config time \
	&& rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo
ENV PATH=/usr/local/cargo/bin:$PATH

# `--default-toolchain none`: the toolchain itself comes from
# rust-toolchain.toml below, resolving whatever `stable` is the day this
# image is built — never a version pinned in this file.
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
	| sh -s -- -y --default-toolchain none --profile minimal

COPY rust-toolchain.toml /tmp/toolchain/rust-toolchain.toml
RUN cd /tmp/toolchain && rustup show && rm -rf /tmp/toolchain

WORKDIR /workspace
