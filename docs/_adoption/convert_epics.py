#!/usr/bin/env python3
"""Convert flat docs/epics/epic-N-*.md into canonical folder layout.
One folder per epic, one file per task, DESCRIPTION.md with computed wave plan.
Does NOT delete originals or touch README (done after manual verification).
"""
import re, sys, pathlib

EPICS = pathlib.Path("docs/epics")
flat = sorted(EPICS.glob("epic-*.md"))

# regex
H1 = re.compile(r'^#\s+(.*)$')
TASK_HD = re.compile(r'^###\s+(T[\w.\-]+)\s*(?:\[([^\]]+)\])?\s*(.*?)\s*$')
L2 = re.compile(r'^##\s+(.*)$')
DEPS_BULLET = re.compile(r'^\s*-\s*\*\*Dependencies?\*\*\s*:\s*(.*)$', re.I)
TASKID = re.compile(r'T[0-9]+(?:\.[0-9]+|\.[A-Za-z]+-[0-9]+|\.[A-Za-z0-9\-]+)?')

report = []

for fp in flat:
    text = fp.read_text(encoding="utf-8")
    lines = text.splitlines()
    stem = fp.stem  # e.g. epic-0-bootstrap
    title = ""
    # find H1
    i = 0
    while i < len(lines):
        m = H1.match(lines[i])
        if m:
            title = m.group(1).strip(); i += 1; break
        i += 1
    # preamble: until first ### or ##
    preamble = []
    while i < len(lines) and not lines[i].startswith("### ") and not lines[i].startswith("## "):
        preamble.append(lines[i]); i += 1
    # now parse: sequence of ### task blocks, possibly interrupted by ## epic-level sections (tail)
    tasks = []   # (id, tag, ttitle, body_lines)
    tail = []    # epic-level ## sections (raw)
    cur = None
    mode = "task"
    while i < len(lines):
        ln = lines[i]
        tm = TASK_HD.match(ln)
        l2 = L2.match(ln)
        if tm:
            if cur: tasks.append(cur)
            cur = [tm.group(1), (tm.group(2) or "").strip(), tm.group(3).strip(), []]
            mode = "task"
        elif l2 and mode == "task" and cur is not None:
            # entering epic-level tail section after tasks
            if cur: tasks.append(cur); cur = None
            mode = "tail"; tail.append(ln)
        elif l2 and mode != "task":
            tail.append(ln)
        else:
            if mode == "task" and cur is not None:
                cur[3].append(ln)
            elif mode == "tail":
                tail.append(ln)
            # lines before any task and not preamble (rare) -> ignore
        i += 1
    if cur: tasks.append(cur)

    task_ids = [t[0] for t in tasks]
    idset = set(task_ids)

    # parse deps per task (all ids found in Dependencies bullet)
    deps = {}
    for tid, tag, tt, body in tasks:
        d = []
        for bl in body:
            mb = DEPS_BULLET.match(bl)
            if mb:
                d = TASKID.findall(mb.group(1))
                break
        deps[tid] = d

    # compute waves using ONLY intra-epic deps
    intra = {t: [x for x in deps[t] if x in idset] for t in task_ids}
    placed = {}
    remaining = set(task_ids)
    wave = 0
    waves = []
    guard = 0
    while remaining and guard < 100:
        guard += 1
        wave += 1
        ready = [t for t in remaining if all(dep in placed for dep in intra[t])]
        if not ready:  # cycle / unresolved -> dump rest
            ready = list(remaining)
        # preserve original order
        ready = [t for t in task_ids if t in ready]
        for t in ready:
            placed[t] = wave
        waves.append((wave, ready))
        remaining -= set(ready)

    # ---- write folder ----
    folder = EPICS / stem
    folder.mkdir(exist_ok=True)

    # DESCRIPTION.md
    desc = [f"# {title}", ""]
    pre = "\n".join(preamble).strip()
    if pre:
        desc.append(pre); desc.append("")
    desc.append("## Dependency graph & parallelism plan"); desc.append("")
    for wnum, ws in waves:
        annot = []
        for t in ws:
            ext = [x for x in deps[t] if x not in idset]
            ia = intra[t]
            if ia or ext:
                annot.append(f"{t}(deps: {', '.join(ia+['ext:'+e for e in ext])})")
        kind = "parallel" if len(ws) > 1 else "single"
        desc.append(f"Wave {wnum} ({kind}): {', '.join(ws)}")
    desc.append("")
    # preserve original epic-level tail (graph/serialization notes) verbatim, demoted
    tail_txt = "\n".join(tail).strip()
    if tail_txt:
        desc.append("## Notes from original epic doc (preserved)")
        desc.append("")
        # demote any ## inside tail to ### to avoid clashing top-level headers
        for tl in tail:
            if tl.startswith("## "):
                desc.append("### " + tl[3:])
            else:
                desc.append(tl)
        desc.append("")
    (folder / "DESCRIPTION.md").write_text("\n".join(desc).rstrip() + "\n", encoding="utf-8")

    # task files
    for tid, tag, tt, body in tasks:
        d = deps[tid]
        # strip the Dependencies bullet from body (replaced by deps: line); keep rest
        nb = [bl for bl in body if not DEPS_BULLET.match(bl)]
        # trim leading/trailing blank lines and stray horizontal-rule separators
        def _strip(seq):
            while seq and (not seq[0].strip() or seq[0].strip() == "---"): seq.pop(0)
            while seq and (not seq[-1].strip() or seq[-1].strip() == "---"): seq.pop()
        _strip(nb)
        out = [f"# {tt}".rstrip(), ""]
        if tag: out.append(f"**Tag**: {tag}")
        if d:   out.append(f"deps: {', '.join(d)}")
        else:   out.append("deps:")
        out.append("")
        out.extend(nb)
        (folder / f"{tid}.md").write_text("\n".join(out).rstrip() + "\n", encoding="utf-8")

    report.append((stem, title, task_ids, [w for _, w in waves], bool(tail_txt)))

# ---- report ----
print("CONVERSION REPORT (originals NOT deleted; README NOT touched)\n")
total = 0
for stem, title, tids, waves, has_tail in report:
    total += len(tids)
    print(f"{stem}/  — {title}")
    print(f"   tasks ({len(tids)}): {', '.join(tids)}")
    print(f"   waves: " + " | ".join(f"W{n+1}:[{','.join(w)}]" for n,w in enumerate(waves)))
    print(f"   tail-preserved: {has_tail}")
print(f"\nTOTAL tasks across {len(report)} epics: {total}")
