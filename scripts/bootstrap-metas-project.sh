#!/usr/bin/env bash
set -euo pipefail
wiki_dir="$(cd "$(dirname "$0")/.." && pwd)"
exec bash "$wiki_dir/.metas-platform/platform/integrations/llm-wiki/scripts/bootstrap-metas-project.sh" "$@"
