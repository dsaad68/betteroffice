#!/usr/bin/env bash
# Prepare a freshly created worktree for the pptx render improvement harness.
# Intended to run from wt's post-start hook, after `wt step copy-ignored`.
# Safe to re-run, and a no-op on branches that do not carry the harness.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

if [ ! -d render-improvement-harness ]; then
  echo "no render-improvement-harness/ on this branch; nothing to set up"
  exit 0
fi

# A copied .venv carries a .pth naming the worktree it was built in, so the
# binding would silently resolve to that other tree's compiled .so. Drop it
# before rebuilding: a failed build must break the import loudly rather than
# leave the harness rendering with another worktree's engine. The python
# version is globbed so this keeps working when the interpreter changes.
for pth in .venv/lib/python*/site-packages/betteroffice_pptx.pth; do
  [ -f "$pth" ] || continue
  if ! grep -qx "$root/bindings/python-pptx/python" "$pth"; then
    echo "dropping stale binding path: $(cat "$pth")"
    rm -f "$pth"
  fi
done

if [ ! -x .venv/bin/python ]; then
  echo "creating .venv"
  python3 -m venv .venv
fi
if ! .venv/bin/python -c "import PIL, numpy, yaml" 2>/dev/null || [ ! -x .venv/bin/maturin ]; then
  echo "installing python dependencies"
  .venv/bin/pip install -q --upgrade pip
  .venv/bin/pip install -q pillow numpy pyyaml maturin
fi

echo "building betteroffice_pptx from this worktree (first build is slow; the copied target/ keeps it incremental)"
(cd bindings/python-pptx && "$root/.venv/bin/maturin" develop)

# The binding must resolve inside this worktree, or every render would measure
# the wrong engine.
resolved="$(.venv/bin/python -c 'import betteroffice_pptx as b; print(b.__file__)')"
case "$resolved" in
  "$root"/*) echo "binding ok: $resolved" ;;
  *) echo "ERROR: binding resolves outside this worktree: $resolved" >&2; exit 1 ;;
esac
.venv/bin/python -c 'import betteroffice_pptx as b; assert hasattr(b.Presentation, "render_png"); print("render_png present")'

decks="$(ls -d render-improvement-harness/decks/*/ 2>/dev/null | wc -l | tr -d ' ')"
sources="$(ls render-improvement-harness/decks/*/source.pptx 2>/dev/null | wc -l | tr -d ' ')"
refs="$(ls -d render-improvement-harness/decks/*/lo-img 2>/dev/null | wc -l | tr -d ' ')"
echo "harness: $decks deck(s) registered, $sources source file(s), $refs LibreOffice reference set(s)"
if [ "$sources" -lt "$decks" ]; then
  echo "note: decks without source.pptx cannot be re-rendered; re-add them with scripts/add_deck.py"
fi
echo "re-render one deck with: .venv/bin/python render-improvement-harness/scripts/pipeline.py <deck-id> --skip-lo"
