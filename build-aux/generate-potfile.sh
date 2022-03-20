#!/bin/sh

src="$(find src/ -path '*.rs')"
git ls-files \
	$src "data/resources/*.ui" "data/*.desktop.in.in" "data/*.xml.in.in" \
	> po/POTFILES.in

cd po || exit 1
intltool-update --maintain 2> /dev/null || echo "Error running intltool"; exit 1
cat missing | grep '^\(src\|data\)/'
code=$?
rm missing

if [ $code -eq 0 ]
then
	exit 1
fi
