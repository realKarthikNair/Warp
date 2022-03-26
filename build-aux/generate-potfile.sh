#!/bin/sh

src="$(find src/ -path '*.rs')"
git ls-files \
	$src "data/resources/*.ui" "data/*.desktop.in.in" "data/*.xml.in.in" \
	> po/POTFILES.in

cd po || exit 1
intltool-update --maintain 2> /dev/null || (echo "Error running intltool"; exit 1)
cd ..
xgettext --keyword=gettextf --keyword=ngettextf:1,2 --keyword=ngettextf_:1,2 --from-code=utf-8 --files-from=po/POTFILES.in -o po/warp.pot 2>/dev/null || (echo "Error running xgettext"; exit 1)
