#!/bin/bash
set -euo pipefail

LANG=$1
BRANCH_NAME="ci/${LANG}/regen-$(date +%s)"

# Check if PR already exists
EXISTING_PR=$(gh pr list --head "ci/${LANG}/regen-" --json number --jq '.[0].number // empty')

if [ -n "$EXISTING_PR" ]; then
    echo "Updating existing PR #$EXISTING_PR"
    # Update the existing PR
    gh pr edit "$EXISTING_PR" --title "Auto-regenerate $LANG binding" --body "Updated binding for $LANG"
else
    echo "Creating new PR"
    # Create new PR
    gh pr create --title "Auto-regenerate $LANG binding" --body "New binding for $LANG" --label automerge
fi 
