# Warp - Share files with each other effortlessly

Warp is designed to make it as simple and secure as possible to get files from one place to another. An internet 
connection is required.

The best transfer method will be determined using [Magic Wormhole](https://magic-wormhole.readthedocs.io/en/latest/)
which includes local network transfer if possible. Every file transfer is encrypted.

<div align="center">
![File Transfer](data/screenshots/screenshot5.png "File Transfer")
</div>

## Flatpak

Flatpak is the recommended way to build and run Warp.

### Build

Make sure you have `flatpak` and `flatpak-builder` installed. Then run the commands below.

```shell
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user org.gnome.Sdk//42 org.freedesktop.Sdk.Extension.rust-stable//21.08 org.gnome.Platform//42
cd build-aux
flatpak-builder --user app app.drey.Warp.Devel.yaml
```

### Run

Once the project is built, run the command below.

```shell
flatpak-builder --run app app.drey.Warp.Devel.yaml warp
```

### Install

After installing the dependencies you can build and install with this command:

```shell
cd build-aux
flatpak-builder --install --user app app.drey.Warp.Devel.yaml warp --force-clean 
```

## Meson

It is supported to install the project locally without flatpak.

```shell
meson build
cd build
ninja
sudo ninja install
```

To uninstall:

```shell
cd build
ninja uninstall
```

It is required to test any big changes with flatpak before contributing any new code.

# Contributing
See the [Contribution guide](./CONTRIBUTING.md) on how to contribute to the project
