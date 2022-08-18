# Contributing
Contributions of all kind and with all levels of experience are very welcome. Please note that the GNOME Code of Conduct
applies to this project.

## Translation
The translation of Warp is managed by the GNOME Translation Project and the respective language teams. The translation status is available on the module page.

[Translation status](https://l10n.gnome.org/module/warp/)

## Development
### Cargo

Compiling and running the project via cargo is possible. This is mostly helpful when debugging as the round-trip time is
faster. When running via cargo the following features
are not available:

* Help pages
* Translations

It is required to test any big changes with flatpak before contributing any new code.

### Debugging

The log level can be adjusted by setting the `RUST_LOG` variable:

```shell
RUST_LOG=debug cargo run
```

### Release

Creating a release:

- Update version in `Cargo.toml`
- Run `cargo` to update `Cargo.lock`
- Run `build-aux/generate-manifest.bash`
- Update `data/app.drey.Warp.metainfo.xml.in.in`
  - Add/Update release section with current date, version and release notes
- Update flathub repo and create PR
- Test flathub build
- Merge flathub PR
- Tag the release
