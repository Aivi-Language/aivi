#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/gh-actions-check.sh [options]

Checks that GitHub Actions workflows are enabled and that the latest run on the
target branch completed successfully for each workflow.

Options:
  -R, --repo OWNER/REPO   Override repo (default: current repo via gh)
  -b, --branch BRANCH     Override branch (default: repo default branch)
  -L, --limit N           Max runs to search per workflow (default: 20)
      --allow-disabled    Do not fail for disabled workflows (warn only)
      --allow-missing     Do not fail if a workflow has no runs (warn only)
  -h, --help              Show this help
EOF
}

repo=""
branch=""
limit="20"
allow_disabled="0"
allow_missing="0"

while [[ $# -gt 0 ]]; do
  case "$1" in
    -R|--repo) repo="${2:-}"; shift 2 ;;
    -b|--branch) branch="${2:-}"; shift 2 ;;
    -L|--limit) limit="${2:-}"; shift 2 ;;
    --allow-disabled) allow_disabled="1"; shift ;;
    --allow-missing) allow_missing="1"; shift ;;
    -h|--help) usage; exit 0 ;;
    *)
      echo "error: unknown arg: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if ! command -v gh >/dev/null 2>&1; then
  echo "error: gh CLI not found in PATH" >&2
  exit 127
fi

repo_flag=()
if [[ -n "$repo" ]]; then
  repo_flag=(-R "$repo")
fi

if ! gh auth status "${repo_flag[@]}" >/dev/null 2>&1; then
  echo "error: gh is not authenticated; run: gh auth login" >&2
  exit 1
fi

if [[ -z "$repo" ]]; then
  repo="$(gh repo view --json nameWithOwner -q .nameWithOwner)"
  repo_flag=(-R "$repo")
fi

if [[ -z "$branch" ]]; then
  branch="$(gh repo view "${repo_flag[@]}" --json defaultBranchRefName -q .defaultBranchRefName)"
fi

echo "Repo:   $repo"
echo "Branch: $branch"
echo

fail="0"

# Name, path, state
while IFS=$'\t' read -r wf_name wf_path wf_state; do
  [[ -n "$wf_name" ]] || continue

  if [[ "$wf_state" != "active" ]]; then
    if [[ "$allow_disabled" == "1" ]]; then
      echo "WARN  workflow disabled: $wf_name ($wf_path)"
    else
      echo "FAIL  workflow disabled: $wf_name ($wf_path)"
      fail="1"
    fi
    continue
  fi

  # Latest run on target branch for this workflow.
  run_tsv="$(
    gh run list "${repo_flag[@]}" \
      --workflow "$wf_name" \
      --branch "$branch" \
      --limit "$limit" \
      --json status,conclusion,url,createdAt,event,headSha \
      --template '{{range .}}{{.status}}{{"\t"}}{{.conclusion}}{{"\t"}}{{.event}}{{"\t"}}{{.createdAt}}{{"\t"}}{{.headSha}}{{"\t"}}{{.url}}{{"\n"}}{{end}}' \
      | head -n 1 \
      || true
  )"

  if [[ -z "$run_tsv" ]]; then
    if [[ "$allow_missing" == "1" ]]; then
      echo "WARN  no runs found:      $wf_name"
    else
      echo "FAIL  no runs found:      $wf_name"
      fail="1"
    fi
    continue
  fi

  IFS=$'\t' read -r run_status run_conclusion run_event run_created run_sha run_url <<<"$run_tsv"

  if [[ "$run_status" != "completed" ]]; then
    echo "FAIL  not completed:      $wf_name ($run_status) $run_url"
    fail="1"
    continue
  fi

  if [[ "$run_conclusion" != "success" ]]; then
    echo "FAIL  last run $run_conclusion: $wf_name ($run_event $run_created ${run_sha:0:7}) $run_url"
    fail="1"
    continue
  fi

  echo "OK    $wf_name ($run_event $run_created ${run_sha:0:7})"
done < <(
  gh workflow list "${repo_flag[@]}" --all \
    --json name,path,state \
    --template '{{range .}}{{.name}}{{"\t"}}{{.path}}{{"\t"}}{{.state}}{{"\n"}}{{end}}'
)

echo
if [[ "$fail" == "1" ]]; then
  echo "One or more workflows are failing or missing runs." >&2
  exit 1
fi

echo "All workflows look healthy."
