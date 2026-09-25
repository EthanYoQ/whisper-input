#!/usr/bin/env bash
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/bin" "$tmp/work/release-assets" "$tmp/work/promotion-evidence"

# Execute the actual workflow step, so the mock checks the code that will run in Actions.
python3 - "$repo/.github/workflows/release-promotion.yml" "$tmp/work/promote.sh" <<'PY'
import pathlib, sys
lines = pathlib.Path(sys.argv[1]).read_text(encoding='utf-8').splitlines()
start = next(i for i, line in enumerate(lines) if line.strip() == '- name: Create or update the Chinese GitHub Release')
start = next(i for i in range(start, len(lines)) if lines[i].strip() == 'run: |') + 1
end = next(i for i in range(start, len(lines)) if lines[i].startswith('      - name: '))
pathlib.Path(sys.argv[2]).write_text('\n'.join(line[10:] for line in lines[start:end]) + '\n', encoding='utf-8')
PY

cat > "$tmp/bin/gh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
case "$1 $2" in
  'release view')
    test -f "$MOCK_RELEASE" || exit 1
    if [[ " $* " == *' --json '* ]]; then cat "$MOCK_RELEASE"; fi
    ;;
  'release create')
    echo create >> "$MOCK_WRITES"
    shift 2
    while (($#)); do
      case "$1" in
        --target) target="$2"; shift 2 ;;
        --notes-file) notes="$2"; shift 2 ;;
        *) shift ;;
      esac
    done
    assets="$(for file in release-assets/*; do jq -n --arg name "$(basename "$file")" --arg digest "sha256:$(sha256sum "$file" | cut -d' ' -f1)" '{name:$name,digest:$digest}'; done | jq -s '.')"
    jq -n --arg body "$(cat "$notes")" --arg target "$target" --argjson assets "$assets" \
      '{body:$body,targetCommitish:$target,assets:$assets,isDraft:true,isPrerelease:false}' > "$MOCK_RELEASE"
    ;;
  'release upload')
    echo upload >> "$MOCK_WRITES"
    file="$4"
    jq --arg name "$(basename "$file")" --arg digest "sha256:$(sha256sum "$file" | cut -d' ' -f1)" \
      '.assets += [{name:$name,digest:$digest}]' "$MOCK_RELEASE" > "$MOCK_RELEASE.next"
    mv "$MOCK_RELEASE.next" "$MOCK_RELEASE"
    ;;
  'release edit')
    echo edit >> "$MOCK_WRITES"
    if [[ " $* " == *' --draft=false'* ]]; then
      test ! -f "$MOCK_FAIL_PUBLISH" || exit 17
      jq '.isDraft = false' "$MOCK_RELEASE" > "$MOCK_RELEASE.next"
    else
      notes=''
      while (($#)); do
        if [[ "$1" == '--notes-file' ]]; then notes="$2"; break; fi
        shift
      done
      jq --arg body "$(cat "$notes")" '.body = $body' "$MOCK_RELEASE" > "$MOCK_RELEASE.next"
    fi
    mv "$MOCK_RELEASE.next" "$MOCK_RELEASE"
    ;;
  *) echo "Unexpected gh call: $*" >&2; exit 2 ;;
esac
SH
chmod +x "$tmp/bin/gh"

export PATH="$tmp/bin:$PATH"
export MOCK_RELEASE="$tmp/release.json" MOCK_WRITES="$tmp/writes" MOCK_FAIL_PUBLISH="$tmp/fail-publish"
export CONTRACT_PATH="$repo/release-contracts/v1.5.4.json"
export RELEASE_TAG="$(jq -r '.release.tag' "$CONTRACT_PATH")"
export SOURCE_COMMIT="$(jq -r '.release.sourceCommit' "$CONTRACT_PATH")"
export GITHUB_REPOSITORY='EthanYoQ/whisper-input'
while IFS= read -r asset; do printf '%s' "$asset" > "$tmp/work/release-assets/$asset"; done < <(jq -r '.release.macos.assets[], .release.windows.assets[]' "$CONTRACT_PATH")
(cd "$tmp/work" && sha256sum release-assets/* > promotion-evidence/asset-sha256s.txt)

run_promotion() { (cd "$tmp/work" && bash promote.sh); }
expect_write_free_failure() {
  : > "$MOCK_WRITES"
  if run_promotion > "$tmp/output" 2>&1; then cat "$tmp/output"; echo 'Expected preflight rejection' >&2; exit 1; fi
  test ! -s "$MOCK_WRITES" || { cat "$MOCK_WRITES"; echo 'Preflight performed a write' >&2; exit 1; }
}

touch "$MOCK_FAIL_PUBLISH"
if run_promotion > "$tmp/output" 2>&1; then echo 'Expected publish failure' >&2; exit 1; fi
test "$(jq -r '.isDraft' "$MOCK_RELEASE")" = true
test "$(grep -c '^create$' "$MOCK_WRITES")" = 1
rm "$MOCK_FAIL_PUBLISH"
: > "$MOCK_WRITES"
run_promotion
test "$(jq -r '.isDraft' "$MOCK_RELEASE")" = false
! grep -q '^create$\|^upload$' "$MOCK_WRITES"

jq '.isDraft = true | .body = "manual draft"' "$MOCK_RELEASE" > "$tmp/changed.json"
cp "$tmp/changed.json" "$MOCK_RELEASE"
expect_write_free_failure

# Restore this workflow's own draft, then reject extra assets and mismatched bytes.
jq --arg body "$(cat "$tmp/work/release-notes.zh-CN.md")" '.body = $body | .targetCommitish = "wrong-source"' "$MOCK_RELEASE" > "$tmp/changed.json"
cp "$tmp/changed.json" "$MOCK_RELEASE"
expect_write_free_failure
jq --arg source "$SOURCE_COMMIT" '.targetCommitish = $source | .assets += [{name:"extra.zip",digest:"sha256:deadbeef"}]' "$MOCK_RELEASE" > "$tmp/changed.json"
cp "$tmp/changed.json" "$MOCK_RELEASE"
expect_write_free_failure
jq '.assets |= map(select(.name != "extra.zip")) | .assets[0].digest = "sha256:deadbeef"' "$MOCK_RELEASE" > "$tmp/changed.json"
cp "$tmp/changed.json" "$MOCK_RELEASE"
expect_write_free_failure
echo 'release promotion retry and write-free rejection: PASS'
