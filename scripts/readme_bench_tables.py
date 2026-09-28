#!/usr/bin/env python3
"""Render the README benchmark block from target/criterion.

Time and ratio sit in separate columns so the × column scans on its own.
A library column pair is omitted when every row is an em dash. A row is
omitted when no competitor has a value. Ratios are competitor/bitflagset
on the printed times: one decimal below 100, an integer at 100 or above.
Italics mark a ratio whose raw median is slower for bitflagset.
"""

import json
import os
import sys

EM = "—"
ROOT = os.environ["REPO_ROOT"]
DATE = os.environ["BENCH_DATE"]
LOAD = os.environ["BENCH_LOAD"]

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

# Short cause for a slower cell. Key is (operation label, library).
CAUSES = {
    ("contains / get", "bitvec"): "one-word load",
    ("contains / get", "bit-set"): "one-word load",
    ("contains / get", "bit-vec"): "one-word load",
    ("insert", "bit-set"): "sets one bit in a pre-sized buffer",
    ("remove (absent)", "bit-set"): "clears one clear bit",
    ("remove (present)", "bit-set"): "clears one set bit",
    ("set", "bit-vec"): "sets one bit in a pre-sized buffer",
    ("set_false (absent)", "bit-vec"): "clears one clear bit",
    ("set_false (present)", "bit-vec"): "clears one set bit",
    ("iter", "bit-set"): "both sum word popcounts",
    ("iter", "bitvec"): "both walk set bits",
    ("clear", "bit-set"): "both memset",
    ("clear", "bit-vec"): "both memset",
    ("union_with / or", "bit-vec"): "both stream NEON orr",
    ("intersect_with / and", "bit-vec"): "NEON and, plus a changed-bit flag",
    ("difference_with", "bit-vec"): "NEON bic, plus a changed-bit mask",
    ("contains", "bitvec"): "reloads &index; taken bounds branch; 256 interleaved median 1.14 (0.78-1.66)",
    ("len", "bitvec"): "relaxed load plus popcount per word",
    ("insert", "bitvec"): "fresh set, then one atomic bit set",
    ("is_empty", "bitvec"): "returns on the first non-zero word",
}


def median_ns(group, bench):
    path = os.path.join(ROOT, "target", "criterion", group, bench, "new", "estimates.json")
    if not os.path.isfile(path):
        return None
    with open(path, encoding="utf-8") as fh:
        return json.load(fh)["median"]["point_estimate"]


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
        ours_ns = median_ns(group, ours_bench) if ours_bench else None
        comps = {}
        any_comp = False
        for lib in LIBS:
            bench = libs.get(lib)
            their_ns = median_ns(group, bench) if bench else None
            if ours_ns is None or their_ns is None:
                comps[lib] = None
                continue
            any_comp = True
            ours_fmt = format_time(ours_ns)
            their_fmt = format_time(their_ns)
            comps[lib] = {
                "time": their_fmt,
                "ratio": ratio_cell(ours_ns, their_ns, ours_fmt, their_fmt),
                "ours_ns": ours_ns,
                "their_ns": their_ns,
            }
        if not any_comp:
            continue
        built.append(
            {
                "label": label,
                "ours": format_time(ours_ns) if ours_ns is not None else EM,
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
                    CAUSES.get((row["label"], lib), "higher median on this sample"),
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
    intro = f"""All numbers below are Criterion medians from `cargo bench --bench compare`, run on Apple M-series (AArch64), collected on **{DATE}**. This is a shared-machine measurement.  
{LOAD}
Each competitor is two columns: its median, then `×` = `their time / bitflagset` on the **printed** times (one decimal, or an integer at 100 or above, e.g. `2.3×`, `1202×`). The bitflagset time and every competitor in that row come from the same Criterion group. A ratio in italics is a row where bitflagset's raw median is slower. `—` means that library has no equivalent in the bench. A column pair that is `—` on every row is dropped, and a row with no competitor value is dropped.  
`iter`, `union`, `intersection`, and `difference` count iterator items. `insert`, `remove`, `set`, `set_false`, `clear`, `union_with` / `or`, `intersect_with` / `and`, and `difference_with` clone the destination in `iter_batched_ref` setup and time only the operation. Those in-place ops borrow the other set (`union_from`, `intersect_from`, `difference_from`), matching bit-set `*_with` and bit-vec `or` / `and` / `difference`. Fixed-size rows use `BatchSize::SmallInput`; the 65536-bit rows use `BatchSize::LargeInput`. `len` / `count` is `len` / `count` / `count_ones`. `clear` is `clear` / `make_empty` / `fill(false)`. `contains` / `get` is `contains` / `get`. bitvec `bitor` / `bitand` / `bitxor` / `not` are by-value operators that build a new set. Atomic `insert` builds a fresh set and sets one bit (`set_aliased` on bitvec). On the 256/1024/65536 tables, `(absent)` uses a clear bit and `(present)` uses a set bit.

### Non-atomic

{non_atomic.rstrip()}

`first` / `last` are omitted: `bit-set` and `bit-vec` have neither. bit-set has no owned `|` / `&` / `-`; those producing operators are the bitvec `bitor` / `bitand` / `bitxor` / `not` rows. Iterator `union` / `intersection` / `difference` allocate nothing and count yielded indices. `union_with` / `or`, `intersect_with` / `and`, and `difference_with` are in place (`union_from`, `intersect_from`, `difference_from` versus bit-set `*_with` and bit-vec `or` / `and` / `difference`). `bit-vec` `insert` / `remove` shift the vector, so membership writes are `set` / `set_false`. `bit-vec` has no `is_subset` and no set-index iterator.

Slower rows:

{markdown_slower(slow_na)}

### Atomic

{atomic.rstrip()}

`is_empty` returns on the first non-zero word. Atomic `contains` loads the word with `Relaxed` ordering. The 65536-bit atomic `insert` row is omitted: that bench has no aliased set on `BitVec<AtomicU64>`, so bitvec has no value there.

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


def main():
    text = render()
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
