# Sequence: Knowledge Base curation

Goal: keep the KB clean, current, and findable as the project grows.

## When to use
- After a batch of completed Tasks (e.g., end of Epic, end of milestone).
- When KB feels noisy or contradictory.
- On request.

## Inputs
- Current state of the KB.

## Steps

1. **kb-curator** → audits the KB:
   - Dedup overlapping entries
   - Reorganize and update the index/TOC
   - Roll completed Task docs into stable docs; archive originals
   - Flag suspected stale/contradictory entries
2. **kb-curator** → applies non-controversial changes; produces a list of flagged items for review
3. For each flagged item:
   - If owned by Architect → ask **gf_architect** or **evo_architect** to confirm/correct
   - If owned by DevOps → ask the relevant **devops-engineer** agent
   - If owned by UI/UX → ask the relevant **ui-ux-designer** agent
   - Update KB based on response

## Output
- Cleanup report (added/merged/archived/removed/reworded)
- Updated KB index
- Resolved flagged entries

## Done when
- KB has no known contradictions.
- Index is accurate.
- Flagged list is empty (or every item has been triaged).
