# Contributing
Contributions of all kind and with all levels of experience are very welcome. Please note that the GNOME Code of Conduct
applies to this project.

## Translation
Warp uses gettext for translation. To generate the required pot file you need to do the following. Replace LANG with
language code of your translation.

```shell
# Create pot file
./build-aux/generate-potfile.sh

# Create a translation file for a new language LANG
cp po/warp.pot po/LANG.po
```

Then you need to add the language code to a new line in the po/LINGUAS file.

Now you can edit LANG.po with any gettext-compatible translation program or with any text editor.

To test out your translation you can need to install the application first. Then you can run it with the
LANGUAGE=LANG environment variable after rebuilding.

```shell
LANGUAGE=LANG flatpak run app.drey.Warp.Devel
```

### Updating translations
To update a translation file (after an application update) you need to generate the pot file first (see above) and then
use the `msgmerge` command:

```shell
msgmerge -U po/LANG.po po/warp.pot
```

## Help pages
The help pages are written in [ducktype](http://projectmallard.org/ducktype/1.0/index.html). The files are stored in
`help/C/duck` and the corresponding `.page`-files can be generated via `make -C help/C/`. Afterwards, you can preview
the generated help pages via `yelp help/C`.

The generated `.page`-files have to be committed to the repository as well. The ducktype program required for running
make is probably packaged in you distro and is also 
[available on GitHub](https://github.com/projectmallard/mallard-ducktype).

### Help page translation
### New language
To generate a new translation for the help pages you can do the following:

```shell
itstool help/C/*.page > help/warp.pot
mkdir help/LANG
cp help/warp.pot help/LANG/LANG.po
```

Then add a new line in `help/LINGUAS` to include the new translation.

### Updating
To update a help file translation

```shell
msgmerge -U help/LANG/LANG.po help/warp.pot
```

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
