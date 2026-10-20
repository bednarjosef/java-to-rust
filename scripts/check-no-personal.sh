#!/usr/bin/env bash
# Fails if committed files or commit identities carry personal data: absolute home paths, or any
# email address other than GitHub noreply ones. Run locally before pushing; CI runs it too.
set -uo pipefail
cd "$(dirname "$0")/.."
rc=0
if git grep -nE '/(home|Users)/[A-Za-z0-9_.-]+' -- . ':!*.lock'; then echo "!! absolute home paths above"; rc=1; fi
if git grep -nE '[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}' -- . ':!*.lock' | grep -vE 'users\.noreply\.github\.com|noreply@github\.com|git@github\.com|example\.(com|org)|@[A-Za-z]+\.(wiki|net|org)/'; then echo "!! email addresses above"; rc=1; fi
if git log --format='%ae%n%ce' | sort -u | grep -vE '^[0-9]+\+[A-Za-z0-9-]+@users\.noreply\.github\.com$|^noreply@github\.com$'; then echo "!! commit identities above are not GitHub noreply addresses"; rc=1; fi
[ $rc -eq 0 ] && echo "no personal data found"
exit $rc
