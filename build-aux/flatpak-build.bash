#!/bin/bash

APP_ID=app.drey.Warp
REPO_DIR=flatpak_repo

if [[ $1 == "dev" ]]; then
  echo "Using devel manifest"
  APP_ID="$APP_ID.Devel"
  MANIFEST="$APP_ID.json"
  RUNTIME_VERSION="master"
elif [[ $1 == "release" ]]; then
  echo "Using release manifest"
  MANIFEST="$APP_ID.yaml"
  RUNTIME_VERSION="$(yq -r '.["runtime-version"]' build-aux/app.drey.Warp.yaml)"
else
  echo "Run either with dev or release as first argument to select the manifest file"
  exit 1
fi

set -xe

build-aux/generate-manifest.bash || echo "Warning: Problem regenerating devel manifest, using existing manifest"

flatpak remote-add --user --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
flatpak remote-add --user --if-not-exists flathub-beta https://flathub.org/beta-repo/flathub-beta.flatpakrepo
flatpak remote-add --user --if-not-exists gnome-nightly https://nightly.gnome.org/gnome-nightly.flatpakrepo

flatpak install --user --noninteractive org.gnome.Sdk//${RUNTIME_VERSION}
flatpak install --user --noninteractive org.gnome.Platform//${RUNTIME_VERSION}

flatpak install --user --noninteractive org.freedesktop.Sdk.Extension.rust-stable//22.08

flatpak-builder \
  --user --verbose --force-clean --install -y --repo=$REPO_DIR flatpak_out build-aux/$MANIFEST
flatpak build-bundle $REPO_DIR $APP_ID.flatpak $APP_ID

flatpak run $APP_ID//master
