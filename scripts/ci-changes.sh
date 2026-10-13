#!/usr/bin/env bash
# Which parts of CI a pull request needs, as GitHub Actions outputs (rust=, web=, docker=).
# Called with the pull request's base commit (and the head, HEAD by default); without a base (a
# push to main, a run by hand, the release) everything runs. Documentation counts for none. A part is skipped only when no changed file can affect it, so a file
# no rule knows runs everything.
set -euo pipefail

base="${1:-}"
head="${2:-HEAD}"
all() { printf 'rust=true\nweb=true\ndocker=true\n'; }
if [[ -z "$base" ]] || ! git cat-file -e "$base^{commit}" 2>/dev/null; then
    all
    exit 0
fi

files=$(git diff --name-only "$base" "$head" |
    grep -vE '^(docs/|LICENSE$|mkdocs\.yml$|requirements-docs\.txt$)|\.md$' || true)
# Files only the UI checks depend on, and files only the Rust jobs depend on
web_only='^(web/|e2e/|package\.json$|pnpm-lock\.yaml$|pnpm-workspace\.yaml$)'
rust_only='^(crates/|Cargo\.toml$|Cargo\.lock$|installer/|scripts/service-smoke\.sh$|\.cargo/)'
# What the image is built from beyond the code the other jobs build and test
docker_files='^(docker/|\.dockerignore$|Cargo\.lock$|package\.json$|pnpm-lock\.yaml$|pnpm-workspace\.yaml$|web/package\.json$|\.github/workflows/ci\.yml$)'

if [[ -z "$files" ]]; then
    printf 'rust=false\nweb=false\ndocker=false\n'
    exit 0
fi
any_outside() { grep -qvE "$1" <<<"$files"; }
echo "rust=$(any_outside "$web_only" && echo true || echo false)"
echo "web=$(any_outside "$rust_only" && echo true || echo false)"
echo "docker=$(grep -qE "$docker_files" <<<"$files" && echo true || echo false)"
