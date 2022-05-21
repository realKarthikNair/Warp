#!/bin/bash

APP_ID=app.drey.Warp
REPO_DIR=flatpak_repo

if [[ $1 == "dev" ]]; then
  echo "Using devel manifest"
  MANIFEST="$APP_ID.Devel.json"
elif [[ $1 == "release" ]]; then
  echo "Using release manifest"
  MANIFEST="$APP_ID.yaml"
else
  echo "Run either with dev or release as first argument to select the manifest file"
  exit 1
fi

set -xe

build-aux/generate-manifest.bash
flatpak-builder \
  --user --verbose --force-clean -y --repo=$REPO_DIR flatpak_out build-aux/$MANIFEST
flatpak build-bundle $REPO_DIR $APP_ID.flatpak $APP_ID
flatpak --user install -y $APP_ID.flatpak
flatpak run $APP_ID//master
