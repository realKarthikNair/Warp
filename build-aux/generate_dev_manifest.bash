#!/usr/bin/env bash

yq -y '
.["app-id"] += ".Devel" |
.["finish-args"] += ["--env=RUST_LOG=warp=debug", "--env=G_MESSAGES_DEBUG=none", "--env=RUST_BACKTRACE=1"] |
.["runtime-version"] = "master" |
.modules |= map(if .name=="warp" then .["sources"] = [{type: "dir", path: "../", skip: ["target", "build", "_build", "builddir", "build-aux/app", ".flatpak", ".flatpak-builder", "build-aux/.flatpak", "build-aux/.flatpak-builder", "flatpak_out", "flatpak_repo", ".fenv"]}] else . end) |
.modules |= map(if .name=="warp" then .["config-opts"] += ["-Dprofile=development"] else . end)' \
build-aux/app.drey.Warp.yml > build-aux/app.drey.Warp.Devel.yml
