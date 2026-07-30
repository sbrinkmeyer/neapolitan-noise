#!/usr/bin/env bash
# Resolve the version to stamp on build artifacts and write it to $GITHUB_OUTPUT.
#
# Precedence: explicit workflow_dispatch tag > pushed tag > native/Cargo.toml.
set -euo pipefail

INPUT_TAG="${1:-}"

if [ -n "$INPUT_TAG" ]; then
  version="$INPUT_TAG"
elif [ "${GITHUB_REF_TYPE:-}" = "tag" ]; then
  version="${GITHUB_REF_NAME}"
else
  version="$(sed -n 's/^version = "\(.*\)"/\1/p' native/Cargo.toml | head -1)"
fi

version="${version#v}"

if [ -z "$version" ]; then
  echo "could not resolve a version" >&2
  exit 1
fi

echo "resolved version: $version"
echo "version=$version" >> "${GITHUB_OUTPUT:-/dev/stdout}"
