---
name: create-issue
description: Use when the user wants to create a GitHub issue — reporting bugs, requesting features, or tracking tasks
---

# Create GitHub Issue

Create a GitHub issue using `gh` CLI for the current repository. Help the user refine their idea into a well-scoped issue through creative discussion.

## Workflow

1. **Extract from context** — if the user already described the issue, extract the core idea, infer label type (bug/feature/etc). Don't ask for info already provided.

2. **Scope refinement** — before jumping to creation, help sharpen the issue:
   - Restate the user's idea in your own words to confirm understanding.
   - If the scope is vague or broad, propose 2-3 concrete framings using `AskUserQuestion` (e.g., "Should this be just the config option, or also include CLI flag override?").
   - Suggest related concerns or edge cases the user might not have considered (e.g., "Should per-vault override global, or merge with it?").
   - If the idea is already clear and narrow — skip straight to step 3, don't over-discuss.

3. **Fetch available labels** from the repo:
```bash
gh label list --limit 50
```
Always use actual label names from the repo — never hardcode or guess label names.

4. **Ask for remaining details** using `AskUserQuestion`:
   - **Label** — offer labels from `gh label list` that are relevant (max 4). Pre-select the most likely one based on context.
   - **Assignee** — offer: `Me (@me)`, `None`

5. **Draft and confirm** — present the full issue (title + body) to the user as formatted text before creating. Let them tweak or approve.

6. **Create the issue:**
```bash
gh issue create --title "TITLE" --label "LABEL" --body "$(cat <<'EOF'
BODY
EOF
)"
```
Use HEREDOC for body to preserve markdown formatting.

7. Return the issue URL to the user.

## Rules

- Always use `gh issue create`, never the GitHub API directly.
- Always run `gh label list` before creating — use exact label names from the repo.
- Body should be concise markdown with `## Description` and relevant subsections (e.g., `## Requirements`, `## Example`).
- Add `--assignee @me` only if user chose to assign to themselves.
- Communicate in the user's language (match their message language).
- Be a creative collaborator, not just a form filler — help the user think through their idea. But don't drag it out if the idea is already well-defined.
