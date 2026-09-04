#!/usr/bin/env bash
# Prepare a freshly created worktree for the pptx render improvement harness.
# Intended to run from wt's post-start hook, after `wt step copy-ignored`.
# Safe to re-run, and a no-op on branches that do not carry the harness.
set -euo pipefail

# The worktree to prepare, given as an argument because this script is invoked from the
# primary worktree: a branch without the harness has no copy of it to run.
root="${1:-$PWD}"
cd "$root" || exit 1

# copy-ignored brings the harness decks and reference renders into every worktree, but the
# ignore rules for them live in render-improvement-harness/.gitignore, which only exists on the
# harness branch. Without this, `git add -A` on a fix branch stages several hundred megabytes of
# third-party decks. info/exclude is shared by every worktree of the repository, and these paths
# are already ignored on the harness branch, so writing it once is harmless there.
exclude="$(git rev-parse --git-common-dir)/info/exclude"
if [ -w "$(dirname "$exclude")" ] && ! grep -q "render-improvement-harness/decks/\*/source.pptx" "$exclude" 2>/dev/null; then
  {
    echo ""
    echo "# Harness inputs copied into every worktree by wt; see render-improvement-harness/.gitignore."
    echo "render-improvement-harness/decks/*/source.pptx"
    echo "render-improvement-harness/decks/*/lo-img/"
    echo "render-improvement-harness/decks/*/bo-img/"
    echo "render-improvement-harness/decks/*/diff-img/"
    echo "render-improvement-harness/decks/*/xml/"
  } >> "$exclude"
  echo "added harness deck paths to $exclude"
fi

if [ ! -d bindings/python-pptx ]; then
  echo "no bindings/python-pptx in $root; nothing to build"
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

# maturin resolves the target environment from VIRTUAL_ENV before it looks at
# its own location. The hook inherits whatever venv the invoking shell had
# active -- typically the primary worktree's -- so without pinning this it
# installs there and repoints that venv at this worktree, breaking both.
export VIRTUAL_ENV="$root/.venv"
export PATH="$root/.venv/bin:$PATH"
unset PYTHONPATH PYTHONHOME
echo "building betteroffice_pptx from this worktree (the copied target/ keeps it incremental)"
(cd bindings/python-pptx && "$root/.venv/bin/maturin" develop)

# The binding must resolve inside this worktree, or every render would measure
# the wrong engine.
resolved="$(.venv/bin/python -c 'import betteroffice_pptx as b; print(b.__file__)')"
case "$resolved" in
  "$root"/*) echo "binding ok: $resolved" ;;
  *) echo "ERROR: binding resolves outside this worktree: $resolved" >&2; exit 1 ;;
esac
.venv/bin/python -c 'import betteroffice_pptx as b; assert hasattr(b.Presentation, "render_png"); print("render_png present")'

if [ ! -d render-improvement-harness ]; then
  echo "binding built; this branch carries no harness, so verify from the harness worktree with:"
  echo "  verify_fix.py <issue-id> --engine $root"
  exit 0
fi

decks="$(ls -d render-improvement-harness/decks/*/ 2>/dev/null | wc -l | tr -d ' ')"
sources="$(ls render-improvement-harness/decks/*/source.pptx 2>/dev/null | wc -l | tr -d ' ')"
refs="$(ls -d render-improvement-harness/decks/*/lo-img 2>/dev/null | wc -l | tr -d ' ')"
echo "harness: $decks deck(s) registered, $sources source file(s), $refs LibreOffice reference set(s)"
if [ "$sources" -lt "$decks" ]; then
  echo "note: decks without source.pptx cannot be re-rendered; re-add them with scripts/add_deck.py"
fi
echo "re-render one deck with: .venv/bin/python render-improvement-harness/scripts/pipeline.py <deck-id> --skip-lo"
