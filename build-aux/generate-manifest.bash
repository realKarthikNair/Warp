#!/usr/bin/env bash
set -e

./build-aux/flatpak-cargo-generator.py -o build-aux/cargo-sources.json Cargo.lock || (echo "error generating cargo sources"; exit 0)

builtin type -P yq &> /dev/null || (echo "yq not found, skipping manifest generation"; exit 0)

yq -o json '
.["id"] += ".Devel" |
.["finish-args"] += ["--env=RUST_LOG=warp=debug", "--env=G_MESSAGES_DEBUG=none", "--env=RUST_BACKTRACE=1"] |
.["runtime-version"] = "master" |
.modules[] |= (
    with(select(.name == "warp");
        .["sources"][0] = {
            "type": "dir",
            "path": "../",
            "skip": [
                "target",
                "build",
                "_build",
                "builddir",
                "build-aux/app",
                ".flatpak",
                ".flatpak-builder",
                "build-aux/.flatpak",
                "build-aux/.flatpak-builder",
                "flatpak_out",
                "flatpak_repo",
                ".fenv"
            ]
        } |
        .["config-opts"] += ["-Dprofile=development"] |
        .["config-opts"] -= ["-Dprofile=default"] |
        .["run-tests"] = true
    )
)' \
build-aux/app.drey.Warp.yaml > build-aux/app.drey.Warp.Devel.json.new

set +e

cmp build-aux/app.drey.Warp.Devel.json build-aux/app.drey.Warp.Devel.json.new > /dev/null

if [[ $? -eq 0 ]]; then
  rm build-aux/app.drey.Warp.Devel.json.new
else
  mv build-aux/app.drey.Warp.Devel.json.new build-aux/app.drey.Warp.Devel.json

  # If the manifest has changed, cargp-about output has probably changed as well
  build-aux/generate-licenses.bash
fi
