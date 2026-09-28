#!/usr/bin/env python3
"""Render the README benchmark block from target/criterion.

Time and ratio sit in separate columns so the × column scans on its own.
A library column pair is omitted when every row is an em dash. A row is
omitted only when every competitor is declared to have no equivalent.
`—` is that declaration. A named benchmark with no Criterion result fails
generation. Ratios are competitor/bitflagset on the printed times: one
decimal below 100, an integer at 100 or above. Italics mark a ratio whose
raw median is slower for bitflagset.
"""

import json
import os
import platform
import shutil
import subprocess
import sys

EM = "—"
ROOT = os.environ.get("REPO_ROOT") or os.path.dirname(
    os.path.dirname(os.path.abspath(__file__))
)
COMMENT_MARKER = "<!-- bench-readme-check -->"

LIBS = ("bitvec", "bit-set", "bit-vec")

# (label, ours bench id, {lib: bench id or None})
NON_ATOMIC_ROWS = [
    ("insert", "ours_insert", {"bit-set": "bitset_insert"}),
    ("remove (absent)", "ours_remove", {"bit-set": "bitset_remove"}),
    ("remove (present)", "ours_remove_present", {"bit-set": "bitset_remove_present"}),
    (
        "contains / get",
        "ours_contains",
        {"bitvec": "bitvec_get", "bit-set": "bitset_contains", "bit-vec": "bit_vec_get"},
    ),
    ("index", "ours_index", {"bitvec": "bitvec_index"}),
    (
        "len / count",
        "ours_len",
        {
            "bitvec": "bitvec_count_ones",
            "bit-set": "bitset_count",
            "bit-vec": "bit_vec_count_ones",
        },
    ),
    ("is_subset", "ours_is_subset", {"bit-set": "bitset_is_subset"}),
    ("iter", "ours_iter", {"bitvec": "bitvec_iter_ones", "bit-set": "bitset_iter"}),
    ("clear", "ours_clear", {"bit-set": "bitset_make_empty", "bit-vec": "bit_vec_fill"}),
    ("set", "ours_set", {"bit-vec": "bit_vec_set"}),
    ("set_false (absent)", "ours_set_false", {"bit-vec": "bit_vec_set_false"}),
    (
        "set_false (present)",
        "ours_set_false_present",
        {"bit-vec": "bit_vec_set_false_present"},
    ),
    ("union", "ours_union", {"bit-set": "bitset_union"}),
    ("intersection", "ours_intersection", {"bit-set": "bitset_intersection"}),
    ("difference", "ours_difference", {"bit-set": "bitset_difference"}),
    (
        "union_with / or",
        "ours_union_from",
        {"bit-set": "bitset_union_with", "bit-vec": "bit_vec_or"},
    ),
    (
        "intersect_with / and",
        "ours_intersect_assign",
        {"bit-set": "bitset_intersect_with", "bit-vec": "bit_vec_and"},
    ),
    (
        "difference_with",
        "ours_difference_assign",
        {"bit-set": "bitset_difference_with", "bit-vec": "bit_vec_difference"},
    ),
    ("bitor", "ours_bitor", {"bitvec": "bitvec_bitor"}),
    ("bitand", "ours_bitand", {"bitvec": "bitvec_bitand"}),
    ("bitxor", "ours_bitxor", {"bitvec": "bitvec_bitxor"}),
    ("not", "ours_not", {"bitvec": "bitvec_not"}),
]

ATOMIC_ROWS = [
    ("len", "atomic_len", {"bitvec": "bitvec_count_ones"}),
    ("is_empty", "atomic_is_empty", {"bitvec": "bitvec_not_any"}),
    ("contains", "atomic_contains", {"bitvec": "bitvec_get"}),
    ("index", "atomic_index", {"bitvec": "bitvec_index"}),
    ("insert", "atomic_insert", {"bitvec": "bitvec_set_aliased"}),
    ("iter", "atomic_iter", {"bitvec": "bitvec_iter_ones"}),
]

NON_ATOMIC_TABLES = [
    ("256-bit", "`[u64; 4]`", "256bit"),
    ("1024-bit", "`[u64; 16]`", "1024bit"),
    ("65536-bit", "`BoxedBitSet`", "65536bit_boxed"),
]

ATOMIC_TABLES = [
    ("256-bit", "`[AtomicU64; 4]`", "256bit_atomic_vs_bitvec"),
    ("1024-bit", "`[AtomicU64; 16]`", "1024bit_atomic_vs_bitvec"),
    ("65536-bit", "`AtomicBoxedBitSet`", "65536bit_atomic_boxed_vs_bitvec"),
]

# (operation label, library) -> cause, only when assembly was actually checked.
# Empty means the slower table prints no per-row cause.
CAUSES = {}


MISSING = []


def median_ns(group, bench):
    path = os.path.join(ROOT, "target", "criterion", group, bench, "new", "estimates.json")
    if not os.path.isfile(path):
        return None
    with open(path, encoding="utf-8") as fh:
        data = json.load(fh)
    try:
        return data["median"]["point_estimate"]
    except (KeyError, TypeError) as exc:
        raise SystemExit(f"bad Criterion estimates at {path}: {exc}") from exc


def take_ns(group, bench, label):
    ns = median_ns(group, bench)
    if ns is None:
        MISSING.append(f"{group}/{bench} ({label})")
    return ns


def ensure_complete():
    if not MISSING:
        return
    print("missing Criterion results:", file=sys.stderr)
    for item in MISSING:
        print(f"  {item}", file=sys.stderr)
    raise SystemExit(2)


def format_time(ns):
    if ns < 1000.0:
        return f"{ns:.2f} ns"
    if ns < 1_000_000.0:
        return f"{ns / 1000.0:.2f} µs"
    return f"{ns / 1_000_000.0:.2f} ms"


def time_to_ns(text):
    num, unit = text.split()
    value = float(num)
    if unit == "ns":
        return value
    if unit == "µs":
        return value * 1000.0
    if unit == "ms":
        return value * 1_000_000.0
    raise SystemExit(f"bad unit in {text!r}")


def format_ratio(ratio):
    # 99.95 prints as 100.0 with one decimal; bump that to an integer.
    if ratio >= 99.95:
        return f"{ratio:.0f}×"
    return f"{ratio:.1f}×"


def ratio_cell(ours_ns, their_ns, ours_fmt, their_fmt):
    shown = time_to_ns(their_fmt) / time_to_ns(ours_fmt)
    text = format_ratio(shown)
    if ours_ns > their_ns:
        text = f"*{text}*"
    return text


def build_rows(group, spec):
    built = []
    for label, ours_bench, libs in spec:
        if not ours_bench:
            raise SystemExit(f"{group} row {label} has no bitflagset benchmark")
        ours_ns = take_ns(group, ours_bench, label)
        comps = {}
        any_declared = False
        for lib in LIBS:
            bench = libs.get(lib)
            if not bench:
                comps[lib] = None
                continue
            any_declared = True
            their_ns = take_ns(group, bench, f"{label} vs {lib}")
            if ours_ns is None or their_ns is None:
                comps[lib] = None
                continue
            ours_fmt = format_time(ours_ns)
            their_fmt = format_time(their_ns)
            comps[lib] = {
                "time": their_fmt,
                "ratio": ratio_cell(ours_ns, their_ns, ours_fmt, their_fmt),
                "ours_ns": ours_ns,
                "their_ns": their_ns,
            }
        if not any_declared:
            continue
        if ours_ns is None:
            continue
        built.append(
            {
                "label": label,
                "ours": format_time(ours_ns),
                "comps": comps,
            }
        )
    return built


def active_libs(rows):
    return [lib for lib in LIBS if any(row["comps"][lib] is not None for row in rows)]


def markdown_table(rows):
    libs = active_libs(rows)
    header = ["Operation", "bitflagset"]
    align = ["l", "r"]
    for lib in libs:
        header.extend([lib, "×"])
        align.extend(["r", "r"])
    body = []
    for row in rows:
        cells = [f"`{row['label']}`", row["ours"]]
        for lib in libs:
            comp = row["comps"][lib]
            if comp is None:
                cells.extend([EM, EM])
            else:
                cells.extend([comp["time"], comp["ratio"]])
        body.append(cells)
    widths = [len(cell) for cell in header]
    for cells in body:
        for i, cell in enumerate(cells):
            widths[i] = max(widths[i], len(cell))
    # GFM wants three hyphens. A right-aligned marker is `---:` (width 4).
    widths = [max(w, 4 if a == "r" else 3) for w, a in zip(widths, align)]

    def fmt(cells):
        padded = []
        for cell, width, side in zip(cells, widths, align):
            padded.append(cell.rjust(width) if side == "r" else cell.ljust(width))
        return "| " + " | ".join(padded) + " |"

    sep_parts = []
    for width, side in zip(widths, align):
        if side == "r":
            sep_parts.append("-" * (width - 1) + ":")
        else:
            sep_parts.append("-" * width)
    lines = [fmt(header), "| " + " | ".join(sep_parts) + " |"]
    lines.extend(fmt(cells) for cells in body)
    return "\n".join(lines)


def slower_rows(size, rows):
    found = []
    for row in rows:
        for lib in active_libs(rows):
            comp = row["comps"][lib]
            if comp is None or not (comp["ours_ns"] > comp["their_ns"]):
                continue
            found.append(
                [
                    f"{size} `{row['label']}` vs {lib}",
                    format_time(comp["ours_ns"]),
                    format_time(comp["their_ns"]),
                    CAUSES.get((row["label"], lib), ""),
                ]
            )
    return found


def markdown_slower(found):
    if not found:
        return "None."
    header = ["Row", "Ours", "Theirs", "Cause"]
    align = ["l", "r", "r", "l"]
    widths = [len(cell) for cell in header]
    for cells in found:
        for i, cell in enumerate(cells):
            widths[i] = max(widths[i], len(cell))
    widths = [max(w, 4 if a == "r" else 3) for w, a in zip(widths, align)]

    def fmt(cells):
        padded = []
        for cell, width, side in zip(cells, widths, align):
            padded.append(cell.rjust(width) if side == "r" else cell.ljust(width))
        return "| " + " | ".join(padded) + " |"

    sep_parts = []
    for width, side in zip(widths, align):
        if side == "r":
            sep_parts.append("-" * (width - 1) + ":")
        else:
            sep_parts.append("-" * width)
    lines = [fmt(header), "| " + " | ".join(sep_parts) + " |"]
    lines.extend(fmt(cells) for cells in found)
    return "\n".join(lines)


def section(tables, specs, kind_note):
    parts = [kind_note, ""]
    slower = []
    for size, storage, group in tables:
        rows = build_rows(group, specs)
        parts.append(f"**{size}** ({storage}):")
        parts.append("")
        parts.append(markdown_table(rows))
        parts.append("")
        slower.extend(slower_rows(size, rows))
    return "\n".join(parts), slower


def render():
    MISSING.clear()
    date = os.environ["BENCH_DATE"]
    load = os.environ["BENCH_LOAD"]
    machine = os.environ["BENCH_MACHINE"]
    non_atomic, slow_na = section(
        NON_ATOMIC_TABLES,
        NON_ATOMIC_ROWS,
        "`BitSet<[u64; N]>` / `BoxedBitSet<u64>` against bitvec `BitArray` / `BitVec<u64>`, "
        "`bit_set::BitSet` (default `u32` blocks, `with_capacity`), and `bit_vec::BitVec` "
        "(default `u32` blocks, `from_elem`). One group per size measures bitflagset and all three libraries.",
    )
    atomic, slow_at = section(
        ATOMIC_TABLES,
        ATOMIC_ROWS,
        "`AtomicBitSet<[AtomicU64; N]>` / `AtomicBoxedBitSet` against bitvec "
        "`BitArray<AtomicU64>` / `BitVec<AtomicU64>`. bit-set and bit-vec have no atomic storage, "
        "so those columns are omitted.",
    )
    ensure_complete()
    intro = f"""All numbers below are Criterion medians from `cargo bench --bench compare`, run on {machine}, collected on **{date}**. This is a shared-machine measurement.  
{load}
Each competitor is two columns: its median, then `×` = `their time / bitflagset` on the **printed** times (one decimal, or an integer at 100 or above, e.g. `2.3×`, `1202×`). The bitflagset time and every competitor in that row come from the same Criterion group. A ratio in italics is a row where bitflagset's raw median is slower. `—` means that library has no equivalent in the bench. A column pair that is `—` on every row is dropped, and a row with no competitor value is dropped.  
`iter`, `union`, `intersection`, and `difference` count iterator items. `insert`, `remove`, `set`, `set_false`, `clear`, `union_with` / `or`, `intersect_with` / `and`, and `difference_with` clone the destination in `iter_batched_ref` setup and time only the operation. Those in-place ops borrow the other set (`union_from`, `intersect_from`, `difference_from`), matching bit-set `*_with` and bit-vec `or` / `and` / `difference`. Fixed-size rows use `BatchSize::SmallInput`; the 65536-bit rows use `BatchSize::LargeInput`. `len` / `count` is `len` / `count` / `count_ones`. `clear` is `clear` / `make_empty` / `fill(false)`. `contains` / `get` is `contains` / `get`. bitvec `bitor` / `bitand` / `bitxor` / `not` are by-value operators that build a new set. Atomic `insert` builds a fresh set and sets one bit (`set_aliased` on bitvec). On the 256/1024/65536 tables, `(absent)` uses a clear bit and `(present)` uses a set bit.

### Non-atomic

{non_atomic.rstrip()}

`first` / `last` are omitted: `bit-set` and `bit-vec` have neither. bit-set has no owned `|` / `&` / `-`; those producing operators are the bitvec `bitor` / `bitand` / `bitxor` / `not` rows. Iterator `union` / `intersection` / `difference` allocate nothing and count yielded indices. `union_with` / `or`, `intersect_with` / `and`, and `difference_with` are in place (`union_from`, `intersect_from`, `difference_from` versus bit-set `*_with` and bit-vec `or` / `and` / `difference`). `bit-vec` `insert` / `remove` shift the vector, so membership writes are `set` / `set_false`. `bit-vec` has no `is_subset` and no set-index iterator.

Slower rows:

{markdown_slower(slow_na)}

### Atomic

{atomic.rstrip()}

`is_empty` returns on the first non-zero word. Atomic `contains` loads the word with `Relaxed` ordering. Atomic `insert`, including the 65536-bit row, builds a fresh set inside the timed closure and sets one bit (`set_aliased` on `BitVec<AtomicU64, Lsb0>`).

Slower rows:

{markdown_slower(slow_at)}

"""
    return intro


def verify(text):
    errors = []
    tables = []
    current = []
    for line_no, line in enumerate(text.splitlines(), 1):
        if line.startswith("|"):
            current.append((line_no, line))
            continue
        if current:
            tables.append(current)
            current = []
    if current:
        tables.append(current)
    if not tables:
        errors.append("no tables found")
    for table in tables:
        width = None
        for line_no, line in table:
            cells = line.split("|")
            # Leading and trailing empties from the wrapping pipes.
            if cells[0].strip() or cells[-1].strip():
                errors.append(f"line {line_no} is not wrapped in pipes")
            inner = cells[1:-1]
            if width is None:
                width = len(inner)
            elif len(inner) != width:
                errors.append(
                    f"line {line_no} has {len(inner)} cells, expected {width}"
                )
        # Skip the separator row (index 1). Data rows of a benchmark table
        # start with a backticked operation. The slower table starts with text.
        header = [c.strip() for c in table[0][1].split("|")[1:-1]]
        if "bitflagset" not in header:
            continue
        ours_i = header.index("bitflagset")
        ratio_indexes = [i for i, name in enumerate(header) if name == "×"]
        for line_no, line in table[2:]:
            cells = [c.strip() for c in line.split("|")[1:-1]]
            ours = cells[ours_i]
            if ours == EM:
                ours_ns = None
            else:
                try:
                    ours_ns = time_to_ns(ours)
                except SystemExit as exc:
                    errors.append(f"line {line_no}: {exc}")
                    continue
            for ratio_i in ratio_indexes:
                time_i = ratio_i - 1
                their = cells[time_i]
                got = cells[ratio_i]
                if their == EM and got == EM:
                    continue
                if their == EM or got == EM:
                    errors.append(f"line {line_no}: time/ratio mismatch {their!r} {got!r}")
                    continue
                if ours_ns is None or ours_ns <= 0:
                    errors.append(f"line {line_no}: ratio without our time")
                    continue
                try:
                    their_ns = time_to_ns(their)
                except SystemExit as exc:
                    errors.append(f"line {line_no}: {exc}")
                    continue
                expect = format_ratio(their_ns / ours_ns)
                shown = got.replace("*", "")
                if shown != expect:
                    errors.append(
                        f"line {line_no}: printed {got} expected {expect} from {their} / {ours}"
                    )
                if got.startswith("*") and not got.endswith("*"):
                    errors.append(f"line {line_no}: broken italics {got}")
    return errors


def _cmd(args):
    try:
        return subprocess.check_output(args, text=True, errors="replace").strip()
    except (OSError, subprocess.CalledProcessError):
        return ""


def runner_info():
    os_name = platform.platform()
    os_release = os.path.join(os.sep, "etc", "os-release")
    if os.path.isfile(os_release):
        with open(os_release, encoding="utf-8") as fh:
            for line in fh:
                if line.startswith("PRETTY_NAME="):
                    os_name = line.split("=", 1)[1].strip().strip('"')
                    break
    cpu = "unknown"
    if shutil.which("lscpu"):
        for line in _cmd(["lscpu"]).splitlines():
            if line.lower().startswith("model name:"):
                cpu = line.split(":", 1)[1].strip()
                break
    elif platform.system() == "Darwin":
        cpu = _cmd(["sysctl", "-n", "machdep.cpu.brand_string"]) or cpu
    if shutil.which("nproc"):
        ncpu = _cmd(["nproc"]) or "?"
    elif platform.system() == "Darwin":
        ncpu = _cmd(["sysctl", "-n", "hw.ncpu"]) or "?"
    else:
        ncpu = "?"
    rustc = _cmd(["rustc", "--version"]) or "unknown"
    sha = _cmd(["git", "-C", ROOT, "rev-parse", "HEAD"]) or "unknown"
    return os_name, f"{cpu} ({ncpu} threads)", rustc, sha


def comparison_counts(rows):
    faster = 0
    slower = 0
    for row in rows:
        for lib in active_libs(rows):
            comp = row["comps"][lib]
            if comp is None:
                continue
            if comp["ours_ns"] < comp["their_ns"]:
                faster += 1
            elif comp["ours_ns"] > comp["their_ns"]:
                slower += 1
    return faster, slower


def render_comment(readme_stale):
    MISSING.clear()
    os_name, cpu, rustc, sha = runner_info()
    faster = 0
    slower = 0
    slow_lines = []
    details = []
    groups = (
        ("non-atomic", NON_ATOMIC_TABLES, NON_ATOMIC_ROWS),
        ("atomic", ATOMIC_TABLES, ATOMIC_ROWS),
    )
    for kind, tables, specs in groups:
        for size, storage, group in tables:
            rows = build_rows(group, specs)
            add_faster, add_slower = comparison_counts(rows)
            faster += add_faster
            slower += add_slower
            for cells in slower_rows(size, rows):
                line = f"- {cells[0]}: {cells[1]} vs {cells[2]}"
                if cells[3]:
                    line = f"{line} — {cells[3]}"
                slow_lines.append(line)
            table = markdown_table(rows)
            details.append(
                "\n".join(
                    [
                        "<details>",
                        f"<summary>{size} {kind} ({storage})</summary>",
                        "",
                        table,
                        "",
                        "</details>",
                    ]
                )
            )
    ensure_complete()
    if slow_lines:
        slow_block = "\n".join(slow_lines)
    else:
        slow_block = "- none"
    stale_block = ""
    if readme_stale:
        stale_block = """
**README tables are stale** relative to the committed data layout. Regenerate and commit:

```bash
cargo bench --bench compare
scripts/update_readme_bench.sh
```
"""
    body = f"""{COMMENT_MARKER}
## bitflagset benchmark report

- OS: {os_name}
- CPU: {cpu}
- rustc: {rustc}
- Commit: `{sha}`

These numbers come from a GitHub-hosted runner and are not comparable to the README's Apple M-series numbers.

bitflagset is faster in {faster} rows and slower in {slower} rows. Slower rows are expected to be noisy on shared runners.

{slow_block}
{stale_block}
{chr(10).join(details)}
"""
    return body


def normalize_structure(text):
    import re

    text = re.sub(r"Slower rows:\n\nNone\.\n", "Slower rows:\n\n<slower>\n", text)
    text = re.sub(r"Slower rows:\n\n(?:\|[^\n]*\n)+", "Slower rows:\n\n<slower>\n", text)
    time_re = re.compile(r"\d+(?:\.\d+)? (?:ns|µs|ms)")
    ratio_re = re.compile(r"\*?\d+(?:\.\d+)?×\*?")
    lines = []
    for line in text.splitlines():
        if line.startswith("|"):
            line = time_re.sub("T", line)
            line = ratio_re.sub("R", line)
        lines.append(line)
    return "\n".join(lines).strip() + "\n"


def structural_diff(readme, generated):
    import difflib

    left = normalize_structure(readme)
    right = normalize_structure(generated)
    if left == right:
        return []
    diff = "".join(
        difflib.unified_diff(
            left.splitlines(keepends=True),
            right.splitlines(keepends=True),
            fromfile="README",
            tofile="generated",
        )
    )
    return ["benchmark block structure differs (numbers ignored):\n" + diff]


def _write_estimate(group, bench, ns):
    path = os.path.join(ROOT, "target", "criterion", group, bench, "new")
    os.makedirs(path, exist_ok=True)
    with open(os.path.join(path, "estimates.json"), "w", encoding="utf-8") as fh:
        json.dump({"median": {"point_estimate": ns}}, fh)


def _fail(msg):
    raise SystemExit(msg)


def self_test():
    import tempfile

    global ROOT
    old_root = ROOT
    tmp = tempfile.mkdtemp(prefix="readme-bench-")
    ROOT = tmp
    os.environ["BENCH_DATE"] = "2026-09-28"
    os.environ["BENCH_LOAD"] = "Load at bench time: before `a`; after `b`."
    os.environ["BENCH_MACHINE"] = "Test CPU (Darwin arm64)"
    try:
        declared = []
        for _size, _storage, group in NON_ATOMIC_TABLES:
            for label, ours, libs in NON_ATOMIC_ROWS:
                declared.append((group, ours, label))
                for lib, bench in libs.items():
                    if bench:
                        declared.append((group, bench, f"{label} {lib}"))
        for _size, _storage, group in ATOMIC_TABLES:
            for label, ours, libs in ATOMIC_ROWS:
                declared.append((group, ours, label))
                for lib, bench in libs.items():
                    if bench:
                        declared.append((group, bench, f"{label} {lib}"))
        for group, bench, _label in declared:
            _write_estimate(group, bench, 10.0)
        text = render()
        if "Test CPU (Darwin arm64)" not in text:
            _fail("machine label was not written")
        if "Apple M-series" in text:
            _fail("machine label is still hardcoded")
        if "`index`" not in text:
            _fail("index row missing from a complete run")
        if "insert` row is omitted" in text:
            _fail("65536 atomic insert is still described as omitted")
        if verify(text):
            _fail(f"rendered block failed verify: {verify(text)}")
        bad = text.replace("1.0×", "9.9×", 1)
        if not verify(bad):
            _fail("verify accepted a ratio that does not match printed times")
        # Same shape, different numbers: not a structural mismatch.
        for group, bench, _label in declared:
            if bench.startswith("bitvec") or bench.startswith("bitset") or bench.startswith("bit_vec"):
                _write_estimate(group, bench, 25.0)
        shifted = render()
        if structural_diff(text, shifted):
            _fail("number changes were reported as a structural mismatch")
        if verify(shifted):
            _fail("shifted block failed ratio check")
        # Drop one declared measurement.
        os.remove(
            os.path.join(
                ROOT,
                "target",
                "criterion",
                "256bit",
                "ours_index",
                "new",
                "estimates.json",
            )
        )
        try:
            render()
        except SystemExit as exc:
            if exc.code != 2:
                _fail(f"missing measurement exited {exc.code!r}, want 2")
        else:
            _fail("missing measurement did not fail")
        # Restore and confirm an undeclared competitor stays an em dash.
        _write_estimate("256bit", "ours_index", 10.0)
        sample = render()
        if "| `insert`" not in sample or "—" not in sample:
            _fail("declared no-equivalent cell did not render an em dash")
        # Row set is structural.
        chopped = "\n".join(
            line for line in sample.splitlines(True) if "`index`" not in line
        )
        if not structural_diff(sample, chopped):
            _fail("dropping a row was not a structural mismatch")
    finally:
        ROOT = old_root
        shutil.rmtree(tmp, ignore_errors=True)
    print("self-test passed")


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "--self-test":
        self_test()
        return
    if len(sys.argv) > 1 and sys.argv[1] == "--structural-diff":
        with open(sys.argv[2], encoding="utf-8") as fh:
            readme = fh.read()
        with open(sys.argv[3], encoding="utf-8") as fh:
            generated = fh.read()
        errors = verify(readme) + verify(generated) + structural_diff(readme, generated)
        if errors:
            for err in errors:
                print(err, file=sys.stderr)
            raise SystemExit(1)
        print("README benchmark block structure matches.")
        return
    comment = "--comment" in sys.argv
    readme_stale = "--readme-stale" in sys.argv
    text = render_comment(readme_stale) if comment else render()
    if not text.endswith("\n"):
        text += "\n"
    errors = verify(text)
    if errors:
        for err in errors:
            print(err, file=sys.stderr)
        sys.exit(1)
    sys.stdout.write(text)


if __name__ == "__main__":
    main()
