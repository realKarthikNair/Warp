#!/usr/bin/python3

try:
    import tomllib
except ImportError:
    from pip._vendor import tomli as tomllib
import sys

with open('Cargo.toml', 'rb') as f:
    toml = tomllib.load(f)
    for arg in sys.argv[1:]:
        toml = toml[arg]
    print(toml, end='')
