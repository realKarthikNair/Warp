#!/bin/bash -xe

APP_ID=net.felinira.warp.Devel
REPO_DIR=flatpak_repo

dirs="./build-aux/.flatpak-builder ./.flatpak-builder ./flatpak_out ./_build ./build"
echo "This will run cargo clean and remove the following directories: '$dirs'"
read -p "Do you want to continue? [y/N]" -n1 -r
if [[ ! $REPLY =~ ^[Yy]$ ]]
then
    exit 1
fi

cargo clean
rm -rf $dirs

flatpak-builder \
  --user --verbose --force-clean -y --repo=$REPO_DIR flatpak_out build-aux/$APP_ID.json
flatpak build-bundle $REPO_DIR $APP_ID.flatpak $APP_ID
flatpak --user install -y $APP_ID.flatpak
flatpak run $APP_ID//master
