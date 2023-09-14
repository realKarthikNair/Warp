#!/usr/bin/python3

import tomllib
import sys

with open('Cargo.toml', 'rb') as f:
    toml = tomllib.load(f)
    for arg in sys.argv[1:]:
        toml = toml[arg]
    print(toml, end='')
