#!/usr/bin/env bash
# Whether main's CI already tested what this release run publishes, as the output `tested=`.
# A release commit from "Bump version" changes only version numbers, so the CI run of the commit
# before it (a push to main, which runs every job) covers the code; release.yml then skips its
# own run of the CI. For anything else (a branch run by hand, a release commit with more in it)
# the commit itself must have a green push run on main, or release.yml runs the CI.
# Needs GH_TOKEN, GITHUB_REPOSITORY, GITHUB_REF and a checkout with the parent commit.
set -euo pipefail

version_files='^(Cargo\.toml|Cargo\.lock|package\.json|web/package\.json|e2e/package\.json)$'

commit=$(git rev-parse HEAD)
if [[ "$GITHUB_REF" == refs/tags/* ]] && git rev-parse -q --verify HEAD^ >/dev/null &&
    ! git diff --name-only HEAD^ HEAD | grep -qvE "$version_files"; then
    commit=$(git rev-parse HEAD^)
    echo "The release commit changes only versions; looking at ${commit:0:7}" >&2
fi

green=$(gh api "repos/$GITHUB_REPOSITORY/actions/workflows/ci.yml/runs?head_sha=$commit&event=push&status=success&per_page=1" \
    --jq '.total_count')
if [[ "$green" -gt 0 ]]; then
    echo "CI was green for ${commit:0:7} on main: not running it again" >&2
    echo "tested=true"
else
    echo "No green CI run on main for ${commit:0:7}: running the CI" >&2
    echo "tested=false"
fi
