#!/usr/bin/env bash

yq -y '
.["app-id"] += ".Devel" |
.["finish-args"] += ["--env=RUST_LOG=warp=debug", "--env=G_MESSAGES_DEBUG=none", "--env=RUST_BACKTRACE=1"] |
.["runtime-version"] = "master" |
.modules |= map(if .name=="warp" then .["sources"] = [{type: "dir", path: "../"}] else . end) |
.modules |= map(if .name=="warp" then .["config-opts"] += ["-Dprofile=development"] else . end)' \
build-aux/net.felinira.warp.yml > build-aux/net.felinira.warp.Devel.yml
