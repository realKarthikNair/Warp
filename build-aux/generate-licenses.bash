#!/usr/bin/env bash
set -e

cargo about -L error generate --all-features --fail --format json |\
    jq 'pick(
        .licenses[].name,
        .licenses[].id,
        .licenses[].text,
        .licenses[].used_by[].crate.name,
        .licenses[].used_by[].crate.authors,
        .licenses[].used_by[].crate.version
    )' > data/licenses.json
