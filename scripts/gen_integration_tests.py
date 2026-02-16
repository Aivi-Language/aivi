#!/usr/bin/env python3
from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
STDLIB_SRC = ROOT / "crates" / "aivi" / "src" / "stdlib"
OUT = ROOT / "integration-tests" / "stdlib"


BUILTIN_TYPE_EXPORTS = {
    "Unit",
    "Bool",
    "Int",
    "Float",
    "Text",
    "Char",
    "Bytes",
    "DateTime",
    "List",
    "Option",
    "Result",
    "Tuple",
    "Map",
    "Set",
    "Queue",
    "Deque",
    "Heap",
    "Source",
    "SourceError",
}


def parse_module_name(rs_text: str) -> str:
    m = re.search(r'MODULE_NAME:\s*&str\s*=\s*"([^"]+)"', rs_text)
    if not m:
        raise ValueError("missing MODULE_NAME")
    return m.group(1)


def parse_aivi_source(rs_text: str) -> str:
    marker = 'pub const SOURCE: &str = r#"'
    start = rs_text.find(marker)
    if start < 0:
        raise ValueError("missing SOURCE marker")
    start += len(marker)
    end = rs_text.find('"#;', start)
    if end < 0:
        raise ValueError("missing SOURCE terminator")
    return rs_text[start:end]


def split_exports(aivi_source: str) -> list[str]:
    out: list[str] = []
    for line in aivi_source.splitlines():
        s = line.strip()
        if not s.startswith("export "):
            continue
        rest = s[len("export ") :].split("//", 1)[0].strip()
        for part in rest.split(","):
            name = part.strip()
            if name:
                out.append(name)
    return out


def is_lower_value(name: str) -> bool:
    return bool(re.match(r"^[a-z][A-Za-z0-9_]*$", name))


def is_upper_name(name: str) -> bool:
    return bool(re.match(r"^[A-Z][A-Za-z0-9_]*$", name))


def is_suffix_template(name: str) -> bool:
    return bool(re.match(r"^[0-9]+[A-Za-z][A-Za-z0-9_]*$", name))


def sanitize_segment(seg: str) -> str:
    seg = seg.strip()
    seg = seg.replace("-", "_")
    seg = re.sub(r"[^A-Za-z0-9_]", "_", seg)
    seg = re.sub(r"_+", "_", seg).strip("_")
    if not seg:
        seg = "x"
    if seg[0].isdigit():
        seg = f"n_{seg}"
    return seg


def module_segments(module_name: str) -> list[str]:
    return [sanitize_segment(s) for s in module_name.split(".")]


def write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def header(module_name: str) -> str:
    return f"@no_prelude\nmodule {module_name}\n"


def use_select(module: str, item: str) -> str:
    return f"use {module} ({item})\n"


def use_all(module: str) -> str:
    return f"use {module}\n"


def use_testlib() -> str:
    return "use aivi.testing (assert)\n"


def parse_type_defs(aivi_source: str) -> set[str]:
    out: set[str] = set()
    for line in aivi_source.splitlines():
        # Types can have parameters: `Table A = ...`, `Vec A = ...`
        m = re.match(r"^\s*([A-Z][A-Za-z0-9_]*)\b.*=", line)
        if m:
            out.add(m.group(1))
    return out


def parse_record_type_aliases(aivi_source: str) -> dict[str, dict[str, str]]:
    out: dict[str, dict[str, str]] = {}
    for line in aivi_source.splitlines():
        # Common stdlib style: `Url = { ... }` or `Table A = { ... }`.
        m = re.match(r"^\s*([A-Z][A-Za-z0-9_]*)\b[^=]*=\s*\{(.*)\}\s*$", line)
        if not m:
            continue
        name = m.group(1)
        body = m.group(2).strip()
        fields: dict[str, str] = {}
        if body:
            for part in body.split(","):
                item = part.strip()
                if not item:
                    continue
                fm = re.match(r"^([a-z][A-Za-z0-9_]*)\s*:\s*(.+)$", item)
                if not fm:
                    continue
                fields[fm.group(1)] = fm.group(2).strip()
        out[name] = fields
    return out


def parse_class_defs(aivi_source: str) -> set[str]:
    out: set[str] = set()
    for line in aivi_source.splitlines():
        m = re.match(r"^\s*class\s+([A-Z][A-Za-z0-9_]*)\b", line)
        if m:
            out.add(m.group(1))
    return out


def parse_ctor_exports(aivi_source: str) -> set[str]:
    # Constructors exported from ADT type declarations, e.g.
    #   ColumnType = IntType | BoolType | Varchar Int
    out: set[str] = set()
    for line in aivi_source.splitlines():
        s = line.strip()
        if not s or s.startswith("//"):
            continue
        if "|" not in s or "=" not in s:
            continue
        m = re.match(r"^([A-Z][A-Za-z0-9_]*)\\b.*=\\s*(.+)$", s)
        if not m:
            continue
        rhs = m.group(2)
        for part in rhs.split("|"):
            head = part.strip().split()
            if not head:
                continue
            name = head[0]
            if is_upper_name(name):
                out.add(name)
    return out


def _split_top_level_commas(text: str) -> list[str]:
    parts: list[str] = []
    cur: list[str] = []
    depth = 0
    for ch in text:
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth = max(0, depth - 1)
        if ch == "," and depth == 0:
            parts.append("".join(cur).strip())
            cur = []
            continue
        cur.append(ch)
    tail = "".join(cur).strip()
    if tail:
        parts.append(tail)
    return parts


def sample_expr_for_type(
    ty: str,
    record_types: dict[str, dict[str, str]],
    domain_templates: list[str],
    domain_named_literals: list[str],
) -> str | None:
    ty = " ".join(ty.strip().split())

    if ty in record_types:
        fields = record_types[ty]
        parts: list[str] = []
        for field, field_ty in fields.items():
            expr = sample_expr_for_type(
                field_ty, record_types, domain_templates, domain_named_literals
            )
            parts.append(f"{field}: {expr or 'Unit'}")
        return "{ " + ", ".join(parts) + " }"

    if ty == "Int":
        return "1"
    if ty == "Float":
        return "1.0"
    if ty == "Bool":
        return "True"
    if ty == "Text":
        return '"x"'
    if ty == "Char":
        return "'x'"
    if ty == "Unit":
        return "Unit"
    if ty == "Bytes":
        return "bytes.empty"

    if ty.startswith("Option "):
        return "None"
    if ty.startswith("List "):
        return "[]"

    if ty.startswith("(") and ty.endswith(")"):
        inner = ty[1:-1].strip()
        items = _split_top_level_commas(inner)
        if len(items) >= 2:
            exprs = [
                sample_expr_for_type(item, record_types, domain_templates, domain_named_literals)
                or "Unit"
                for item in items
            ]
            return "(" + ", ".join(exprs) + ")"

    if ty.startswith("Map "):
        return 'Map.insert "a" 1 Map.empty'
    if ty.startswith("Set "):
        return "Set.insert 1 Set.empty"

    if ty == "Delta":
        if domain_templates:
            tpl = domain_templates[0]
            suffix = re.sub(r"^[0-9]+", "", tpl)
            return "2" + suffix
        if domain_named_literals:
            return domain_named_literals[0]
        return None

    return None


def basic_export_test(
    mod: str,
    export_name: str,
    type_defs: set[str],
    record_types: dict[str, dict[str, str]],
    ctor_exports: set[str],
) -> None:
    mod_segs = module_segments(mod)
    case = sanitize_segment(export_name)
    test_module = ".".join(["integrationTests", "stdlib", *mod_segs, case])
    out_path = OUT / Path(*mod_segs) / f"{case}.aivi"

    body = header(test_module)
    body += "\nuse aivi\n" + use_testlib()
    body += "\n" + use_all(mod)

    if export_name.startswith("domain "):
        return

    if is_upper_name(export_name):
        if export_name in ctor_exports:
            body += f"\nsubject = {export_name}\n"
            body += "\n@test\nsmoke = effect {\n  _ <- pure subject\n  _ <- assert True\n}\n"
        else:
            body += f"\nRef = {export_name}\n"
            body += "\n@test\nsmoke = effect {\n  _ <- assert True\n}\n"
        write(out_path, body)
        return

    subject_expr: str
    if is_suffix_template(export_name):
        suffix = re.sub(r"^[0-9]+", "", export_name)
        subject_expr = ("3.14" + suffix) if suffix in ("dec",) else ("2" + suffix)
    else:
        subject_expr = export_name

    body += f"\nsubject = {subject_expr}\n"
    body += "\n@test\nsmoke = effect {\n  _ <- pure subject\n  _ <- assert True\n}\n"
    write(out_path, body)


@dataclass(frozen=True)
class DomainDef:
    name: str
    carrier: str
    operators: list[tuple[str, str]]  # (op, type_sig)
    templates: list[str]  # e.g. 1ms
    named_literals: list[str]  # e.g. eom


def parse_domain_defs(aivi_source: str) -> list[DomainDef]:
    lines = aivi_source.splitlines()
    out: list[DomainDef] = []
    i = 0
    while i < len(lines):
        line = lines[i]
        m = re.match(
            r"^\s*domain\s+([A-Za-z][A-Za-z0-9_]*)\s+over\s+(.+?)\s*=\s*{\s*$",
            line,
        )
        if not m:
            i += 1
            continue
        name = m.group(1)
        carrier = m.group(2).strip()
        i += 1
        brace = 1
        block: list[str] = []
        while i < len(lines) and brace > 0:
            cur = lines[i]
            brace += cur.count("{")
            brace -= cur.count("}")
            if brace > 0:
                block.append(cur)
            i += 1

        operators: list[tuple[str, str]] = []
        templates: list[str] = []
        named_literals: list[str] = []

        for bline in block:
            m_op = re.match(r"^\s*\(([^)]+)\)\s*:\s*(.+?)\s*$", bline)
            if m_op:
                operators.append((m_op.group(1).strip(), m_op.group(2).strip()))
                continue

            m_tpl = re.match(r"^\s*([0-9]+[A-Za-z][A-Za-z0-9_]*)\s*=\s*.+$", bline)
            if m_tpl:
                templates.append(m_tpl.group(1))
                continue

            m_named = re.match(
                r"^\s*([a-z][A-Za-z0-9_]*)\s*=\s*([A-Z][A-Za-z0-9_]*)\b.*$",
                bline,
            )
            if m_named:
                named_literals.append(m_named.group(1))

        out.append(
            DomainDef(
                name=name,
                carrier=carrier,
                operators=operators,
                templates=templates,
                named_literals=named_literals,
            )
        )
    return out


def domain_tests(mod: str, aivi_src: str, domain: DomainDef) -> None:
    mod_segs = module_segments(mod)
    dom_dir = OUT / Path(*mod_segs) / f"domain_{sanitize_segment(domain.name)}"
    dom_mod_prefix = [
        "integrationTests",
        "stdlib",
        *mod_segs,
        f"domain_{sanitize_segment(domain.name)}",
    ]

    # Operator coverage is currently limited by typechecker ambiguities for multi-carrier domains.
    # We still generate template/named-literal tests to validate delta construction.

    for tpl in domain.templates:
        suffix = re.sub(r"^[0-9]+", "", tpl)
        case = "suffix_" + sanitize_segment(suffix)
        test_module = ".".join([*dom_mod_prefix, case])
        out_path = dom_dir / f"{case}.aivi"

        body = header(test_module)
        body += "\nuse aivi\n" + use_testlib()
        body += "\n" + use_all(mod)
        body += "\n" + use_select(mod, f"domain {domain.name}")
        literal_base = "3.14" if suffix == "dec" else "2"
        body += f"\nsubject = {literal_base}{suffix}\n"
        body += "\n@test\nsmoke = effect {\n  _ <- pure subject\n  _ <- assert True\n}\n"
        write(out_path, body)

    for lit in domain.named_literals:
        case = "literal_" + sanitize_segment(lit)
        test_module = ".".join([*dom_mod_prefix, case])
        out_path = dom_dir / f"{case}.aivi"

        body = header(test_module)
        body += "\nuse aivi\n" + use_testlib()
        body += "\n" + use_all(mod)
        body += "\n" + use_select(mod, f"domain {domain.name}")
        body += f"\nsubject = {lit}\n"
        body += "\n@test\nsmoke = effect {\n  _ <- pure subject\n  _ <- assert True\n}\n"
        write(out_path, body)


def main() -> None:
    if not STDLIB_SRC.exists():
        raise SystemExit(f"missing {STDLIB_SRC}")

    generated = 0
    for rs in sorted(STDLIB_SRC.glob("*.rs")):
        if rs.name == "mod.rs":
            continue
        rs_text = rs.read_text(encoding="utf-8")
        mod = parse_module_name(rs_text)
        aivi_src = parse_aivi_source(rs_text)
        type_defs = parse_type_defs(aivi_src)
        record_types = parse_record_type_aliases(aivi_src)
        class_defs = parse_class_defs(aivi_src)
        ctor_exports = parse_ctor_exports(aivi_src)

        for export in split_exports(aivi_src):
            if export.startswith("domain "):
                continue
            if export in class_defs:
                # Don't generate per-class "value smoke" tests; classes are not values.
                continue
            basic_export_test(mod, export, type_defs, record_types, ctor_exports)
            generated += 1

        for dom in parse_domain_defs(aivi_src):
            domain_tests(mod, aivi_src, dom)

    print(f"generated {generated} export tests under {OUT}")


if __name__ == "__main__":
    main()
