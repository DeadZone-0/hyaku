#!/usr/bin/env bash
set -euo pipefail

usage() {
    printf 'Usage: %s [10|20|30|40|50|60|70|80|90|100] [client options]\n' "$0"
}

if (( $# == 0 )); then
    if [[ -t 0 ]]; then
        read -r -p 'Gatekeeper floor (10, 20, …, 100) [10] ' floor
        floor="${floor:-10}"
    else
        usage >&2
        exit 2
    fi
else
    floor="$1"
    shift
fi

case "$floor" in
    -h|--help) usage; exit 0 ;;
    10|20|30|40|50|60|70|80|90|100) ;;
    *) usage >&2; exit 2 ;;
esac

cd "$(dirname "${BASH_SOURCE[0]}")"
cargo build --quiet -p hyaku-server
exec cargo run --quiet -p hyaku-client -- --raid-floor "$floor" --class 1 "$@"
