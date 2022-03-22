# Warp - Share files with each other effortlessly

Warp is designed to make it as simple and secure as possible to get files from one place to another. An internet 
connection is required.

The best transfer method will be determined using [Magic Wormhole](https://magic-wormhole.readthedocs.io/en/latest/)
which includes local network transfer if possible. Every file transfer is encrypted.

<div align="center">
![Main window](data/resources/screenshots/screenshot1.png "Main window")
</div>

## Building the project

Make sure you have `flatpak` and `flatpak-builder` installed. Then run the commands below.

```
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user org.gnome.Sdk//41 org.freedesktop.Sdk.Extension.rust-stable//21.08 org.gnome.Platform//41
cd build-aux
flatpak-builder --user app net.felinira.warp.Devel.json
```

## Running the project

Once the project is built, run the command below.

```
flatpak-builder --run app net.felinira.warp.Devel.json warp
```

# Contributing
## Translation
Warp uses gettext for translation. To generate the required pot file you need to do the following. Replace LANG with
language code of your translation.

```sh
# Create pot file
xgettext --from-code=utf-8 -o po/warp.pot `cat po/POTFILES.in` 2>/dev/null

# Create a translation file for a new language LANG
cp po/warp.pot po/LANG.po
```

Then you need to add the language code to a new line in the po/LINGUAS file.

Now you can edit LANG.po with any gettext-compatible translation program or with any text editor.

To test out your translation you can need to install the application first. Then you can run the it with the 
LANGUAGE=LANG environment variable after rebuilding.

```
LANGUAGE=LANG flatpak run net.felinira.warp.Devel
```

### Updating translations
To update a translation file (after an application update) you need to generate the pot file first (see above) and then
use the `msgmerge` command:

```
msgmerge -U po/LANG.po po/warp.pot
```

# Attribution
<p>App icon by <a href="https://svgrepo.com">svgrepo.com</a></p>
<p>Symbolic icon made from <a href="http://www.onlinewebfonts.com/icon">Icon Fonts</a> is licensed by CC BY 3.0</p>
