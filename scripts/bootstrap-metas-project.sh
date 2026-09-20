#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
  echo "Usage: $0 ABSOLUTE_PROJECT_PATH [PROJECT_UUID]" >&2
  exit 2
fi

project_path="${1%/}"
project_id="${2:-}"

if [[ "$project_path" != /* || "$project_path" == "/" ]]; then
  echo "Project path must be an absolute, non-root path" >&2
  exit 2
fi

if [[ -z "$project_id" ]]; then
  project_id="$(uuidgen)"
fi

if [[ ! "$project_id" =~ ^[0-9a-fA-F-]{36}$ ]]; then
  echo "Invalid project UUID: $project_id" >&2
  exit 2
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
template_dir="$repo_root/metas-business"

mkdir -p \
  "$project_path/raw/sources" \
  "$project_path/raw/assets" \
  "$project_path/.llm-wiki/chats"

for directory in businesses people tasks operations feedback insights ideas \
  experiments results decisions playbooks metrics sources queries synthesis \
  entities concepts comparisons; do
  mkdir -p "$project_path/wiki/$directory"
done

install -m 0644 "$template_dir/purpose.md" "$project_path/purpose.md"
install -m 0644 "$template_dir/schema.md" "$project_path/schema.md"

if [[ ! -f "$project_path/wiki/index.md" ]]; then
  printf '# Metas Business Knowledge Index\n\nNo sources have been ingested yet.\n' \
    > "$project_path/wiki/index.md"
fi

if [[ ! -f "$project_path/wiki/log.md" ]]; then
  printf '# Ingest Log\n' > "$project_path/wiki/log.md"
fi

if [[ ! -f "$project_path/.llm-wiki/project.json" ]]; then
  printf '{\n  "id": "%s",\n  "createdAt": %s\n}\n' \
    "$project_id" "$(date +%s%3N)" > "$project_path/.llm-wiki/project.json"
fi

printf 'Metas Business project ready: %s\nProject UUID: %s\n' \
  "$project_path" "$project_id"
