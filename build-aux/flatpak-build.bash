#!/bin/bash

APP_ID=net.felinira.warp
REPO_DIR=flatpak_repo

if [[ $1 == "dev" ]]; then
  echo "Using devel manifest"
  APP_ID="$APP_ID.Devel"
elif [[ $1 == "release" ]]; then
  echo "Using release manifest"
else
  echo "Run either with dev or release as first argument to select the manifest file"
  exit 1
fi

dirs="./build-aux/.flatpak-builder ./.flatpak-builder ./flatpak_out ./_build ./build"
echo "This will run cargo clean and remove the following directories: '$dirs'"
read -p "Do you want to continue? [y/N]" -n1 -r
if [[ ! $REPLY =~ ^[Yy]$ ]]
then
    exit 1
fi

cargo clean
rm -rf $dirs

set -xe

flatpak-builder \
  --user --verbose --force-clean -y --repo=$REPO_DIR flatpak_out build-aux/$APP_ID.yml
flatpak build-bundle $REPO_DIR $APP_ID.flatpak $APP_ID
flatpak --user install -y $APP_ID.flatpak
flatpak run $APP_ID//master
