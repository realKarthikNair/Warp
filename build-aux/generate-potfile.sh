#!/bin/sh

src="$(find src/ -path '*.rs')"
ui="$(find src/ -path '*.ui')"
git ls-files \
	$src $ui "data/resources/*.ui" "data/*.desktop.in.in" "data/*.xml.in.in" \
	> po/POTFILES.in

cd po || exit 1
intltool-update --maintain 2> /dev/null || (echo "Error running intltool"; exit 1)
cd ..
xgettext --add-comments --keyword=pgettextf:1c,2 --keyword=npgettextf:1c,2,3 --keyword=gettextf --keyword=ngettextf:1,2 --keyword=ngettextf_:1,2 --from-code=utf-8 --files-from=po/POTFILES.in -o po/warp.pot 2>/dev/null || (echo "Error running xgettext"; exit 1)
