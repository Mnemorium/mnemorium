"""Generate `docs/development/artifacts/RequirementsTraceabilityCoverage.html`, the coverage report of the
specification tree.

Run through `script/investigation.sh`; the module imports no test module, so a conftest fixture chain never runs to
produce a document. It reads:

- `openspec/specs/*/spec.md`, for the declared requirements, their descriptions and their scenarios;
- `test/system/*.py`, for each case's `@scenario("SCN-...", TestType...)` decorators and any `@positive`-style case
  decorator that types every scenario the case declares;
- `test/api/scenarios.toml`, for each scenario the fixture fuzz run proves and the kind of proof it carries;
- `src/**/*.rs`, for each `// proves: SCN-...` comment, the function below it and the kind the comment names after
  its em dash;
- `docs/development/artifacts/RequirementsTraceabilityCoverage.json`, for the annotations a human authored.

The report is a self-contained HTML page rendered with Jinja2. Its charts, summary, requirement descriptions and
scenario steps are generated; the JSON holds only authored text:

- `causes`, mapping a scenario to why it has no proof;
- `not_black_box`, mapping a scenario to why a Rust proof stands in for a black-box case;
- `notes`, mapping a scenario to free-form commentary;
- `not_applicable`, mapping a requirement to the type labels it does not exercise.

The generator reads that file and never writes it. A malformed document or an unknown top-level key is an error; an
identifier that left the specification keeps its annotation and is reported as stale rather than dropped.
"""

from __future__ import annotations

import ast
import html
import json
import math
import re
import sys
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

import markdown as markdown_lib
from jinja2 import Environment, FileSystemLoader, select_autoescape
from markupsafe import Markup

REQUIREMENT_HEADER = re.compile(r"^### Requirement: (?P<identifier>REQ-[A-Z0-9-]+) — (?P<title>.+)$")
SCENARIO_HEADER = re.compile(r"^#### Scenario: (?P<identifier>SCN-[A-Z0-9-]+) — (?P<title>.+)$")
SCENARIO_IDENTIFIER = re.compile(r"^SCN-[A-Z0-9-]+-\d{3}-[A-Z]$")
REQUIREMENT_IDENTIFIER = re.compile(r"^REQ-[A-Z0-9-]+-\d{3}$")
CAPABILITY_OF = re.compile(r"^(?:REQ|SCN)-(?P<capability>.+?)-\d{3}(?:-[A-Z])?$")
RUST_PROVES = re.compile(r"^\s*// proves:\s*(?P<body>.+)$")
RUST_FUNCTION = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+(?P<name>[A-Za-z0-9_]+)")

# The eight types, in the order the requirement matrix declares them (D14).
TYPE_LABELS = {
    "POSITIVE": "Positive",
    "NEGATIVE": "Negative",
    "BOUNDARY": "Boundary",
    "SECURITY": "Security",
    "ERROR_HANDLING": "Error handling",
    "RECOVERY": "Recovery",
    "ALTERNATE_FLOW": "Alternate flow",
    "BUSINESS_RULE": "Business rule",
}
TYPES = tuple(TYPE_LABELS.values())
# A case may declare its kind once for every scenario it proves: the bare decorator's name is the kind (`labels.py`).
CASE_TYPE_DECORATORS = {label.lower().replace(" ", "_"): label for label in TYPES}
# A `// proves:` comment may name its kind after an em dash, either as the label or as the decorator's bare name.
RUST_KINDS = {label: label for label in TYPES} | CASE_TYPE_DECORATORS

ARTEFACT = Path("docs/development/artifacts/RequirementsTraceabilityCoverage.html")
AUTHORED = Path("docs/development/artifacts/RequirementsTraceabilityCoverage.json")
# The scenarios the fixture fuzz run proves. Nothing in that run is a pytest case, so the claims live in this file.
API_MANIFEST = Path("test/api/scenarios.toml")
TEMPLATE_DIR = Path(__file__).with_name("templates")
TEMPLATE_NAME = "requirements_traceability_coverage.html.jinja"

# What a scenario's evidence names. The case and function names stay out of the report: the annotation in the suite is
# their citation, and a renamed test would only churn this page.
SYSTEM_SUITE = "System suite"
API_SUITE = "API suite"
RUST_PROOF = "Proven in Rust"

COVERED = "covered"
SKIPPED = "skipped"
GAP = "gap"
SCENARIO_STATUS = (COVERED, SKIPPED, GAP)
TESTED = "Tested"
PARTIAL = "Partial"
NOT_TESTED = "Not tested"
REQUIREMENT_STATUS = (TESTED, PARTIAL, NOT_TESTED)

COLORS = {
    COVERED: "#2e7d32",
    SKIPPED: "#f9a825",
    GAP: "#c62828",
    TESTED: "#2e7d32",
    PARTIAL: "#ef6c00",
    NOT_TESTED: "#c62828",
    "system": "#1565c0",
    "api": "#6a1b9a",
    "rust": "#00838f",
    "multiple": "#2e7d32",
    "none": "#c62828",
}


@dataclass
class Scenario:
    """One declared scenario, with the requirement and capability its identifier names."""

    identifier: str
    title: str
    requirement: str
    capability: str
    steps: str = ""


@dataclass
class Requirement:
    """One declared requirement, its description and the scenarios beneath it."""

    identifier: str
    title: str
    capability: str
    description: str = ""
    scenarios: list[Scenario] = field(default_factory=list)


@dataclass
class Claim:
    """A system case's declaration that it proves a scenario."""

    scenario: str
    kind: str | None
    test: str


@dataclass
class RustClaim:
    """A Rust function's declaration that it proves a scenario."""

    scenario: str
    location: str
    kind: str | None = None


@dataclass
class Authored:
    """The annotations a human authored in the JSON sidecar."""

    causes: dict[str, str] = field(default_factory=dict)
    not_black_box: dict[str, str] = field(default_factory=dict)
    notes: dict[str, str] = field(default_factory=dict)
    not_applicable: dict[str, list[str]] = field(default_factory=dict)


@dataclass
class Matrix:
    """The parsed state of one generation."""

    requirements: dict[str, Requirement]
    scenarios: dict[str, Scenario]
    claims: list[Claim]
    api_claims: dict[str, str]
    rust_claims: list[RustClaim]
    capabilities: list[str]
    authored: Authored


def capability_of(identifier: str) -> str:
    """The capability an identifier names, as the specification directory spells it in upper case."""
    match = CAPABILITY_OF.match(identifier)
    if match is None:
        raise ValueError(f"{identifier!r} is not a requirement or scenario identifier")
    return match.group("capability")


def display_name(capability: str) -> str:
    """The heading a capability's section carries."""
    if capability == "API":
        return "API"
    return capability.replace("-", " ").title()


def numeric_suffix(identifier: str) -> int:
    """The number of a requirement identifier, for ordering."""
    return int(identifier.rsplit("-", 1)[1])


def requirement_of_scenario(identifier: str) -> str:
    """The requirement a scenario identifier belongs to."""
    return identifier.replace("SCN-", "REQ-", 1).rsplit("-", 1)[0]


def scenario_suffix(identifier: str) -> str:
    """The letter of a scenario identifier, for ordering."""
    return identifier.rsplit("-", 1)[1]


def _percent(value: int, total: int) -> int:
    """A whole-number percentage, with an empty denominator reported as zero."""
    return 0 if total == 0 else round(100 * value / total)


def _percent_text(value: int, total: int) -> str:
    """A percentage with its sign, keeping a non-zero share below one percent visible."""
    if total == 0:
        return "0%"
    percentage = 100 * value / total
    return f"{percentage:.1f}%" if 0 < percentage < 1 else f"{percentage:.0f}%"


def _search_text(*parts: str) -> str:
    """The normalised text a browser-side search matches against."""
    return " ".join(" ".join(part.split()) for part in parts if part).lower()


def parse_specifications(root: Path) -> tuple[dict[str, Requirement], dict[str, Scenario], list[str]]:
    """Read every capability specification into requirements and scenarios indexed by identifier."""
    requirements: dict[str, Requirement] = {}
    scenarios: dict[str, Scenario] = {}
    problems: list[str] = []

    for path in sorted((root / "openspec" / "specs").glob("*/spec.md")):
        current_requirement: Requirement | None = None
        current_scenario: Scenario | None = None
        description: list[str] = []
        steps: list[str] = []

        lines = path.read_text(encoding="utf-8").splitlines()
        for number, line in enumerate(lines, 1):
            requirement_header = REQUIREMENT_HEADER.match(line)
            if requirement_header is not None:
                _finish_scenario(current_scenario, steps)
                _finish_requirement(current_requirement, description)
                identifier = requirement_header.group("identifier")
                if identifier in requirements:
                    problems.append(f"{path}:{number}: duplicate requirement {identifier}")
                current_requirement = Requirement(
                    identifier,
                    requirement_header.group("title"),
                    capability_of(identifier),
                )
                requirements[identifier] = current_requirement
                current_scenario = None
                description = []
                steps = []
                continue

            scenario_header = SCENARIO_HEADER.match(line)
            if scenario_header is not None:
                _finish_requirement(current_requirement, description)
                _finish_scenario(current_scenario, steps)
                identifier = scenario_header.group("identifier")
                current_scenario = None
                if identifier in scenarios:
                    problems.append(f"{path}:{number}: duplicate scenario {identifier}")
                    steps = []
                    continue
                if not SCENARIO_IDENTIFIER.fullmatch(identifier):
                    problems.append(f"{path}:{number}: {identifier} is not a well-formed scenario identifier")
                    steps = []
                    continue
                owner = requirement_of_scenario(identifier)
                if current_requirement is None or current_requirement.identifier != owner:
                    declared = current_requirement.identifier if current_requirement is not None else "no requirement"
                    problems.append(f"{path}:{number}: {identifier} is declared under {declared}, not {owner}")
                    steps = []
                    continue
                current_scenario = Scenario(
                    identifier,
                    scenario_header.group("title"),
                    owner,
                    capability_of(owner),
                )
                scenarios[identifier] = current_scenario
                current_requirement.scenarios.append(current_scenario)
                steps = []
                continue

            if line.startswith(("## ", "### ")):
                _finish_scenario(current_scenario, steps)
                _finish_requirement(current_requirement, description)
                current_requirement = None
                current_scenario = None
                description = []
                steps = []
                continue

            if current_scenario is not None:
                steps.append(line)
            elif current_requirement is not None:
                description.append(line)

        _finish_scenario(current_scenario, steps)
        _finish_requirement(current_requirement, description)

    return requirements, scenarios, problems


def _finish_scenario(scenario: Scenario | None, lines: list[str]) -> None:
    """Give a scenario its body once, when the parser reaches the next heading."""
    if scenario is not None and not scenario.steps:
        scenario.steps = "\n".join(lines).strip()


def _finish_requirement(requirement: Requirement | None, lines: list[str]) -> None:
    """Give a requirement its description once, before its first scenario."""
    if requirement is not None and not requirement.description:
        requirement.description = "\n".join(lines).strip()


def parse_system_claims(root: Path) -> tuple[list[Claim], list[str]]:
    """Read every system case's scenario decorators from the suite's AST."""
    claims: list[Claim] = []
    problems: list[str] = []

    for path in sorted((root / "test" / "system").glob("*.py")):
        tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
        for node in tree.body:
            if not isinstance(node, ast.FunctionDef | ast.AsyncFunctionDef) or not node.name.startswith("test_"):
                continue
            case_type, type_problem = _read_case_type(node)
            if type_problem is not None:
                problems.append(f"{path}:{node.lineno}: {node.name}: {type_problem}")
            declared: list[tuple[str, str | None]] = []
            for decorator in node.decorator_list:
                if not _is_scenario_decorator(decorator):
                    continue
                identifier, kind, problem = _read_scenario_call(decorator)
                if problem is not None:
                    problems.append(f"{path}:{decorator.lineno}: {node.name}: {problem}")
                    continue
                assert identifier is not None
                declared.append((identifier, kind))
            if case_type is not None and not declared:
                problems.append(f"{path}:{node.lineno}: {node.name}: declares a case-level type but no scenario")
            elif case_type is not None and all(own is not None and own != case_type for _, own in declared):
                problems.append(
                    f"{path}:{node.lineno}: {node.name}: declares {case_type}, which every scenario overrides"
                )
            for identifier, kind in declared:
                claims.append(Claim(identifier, kind or case_type, f"{path.name}::{node.name}"))

    return claims, problems


def _case_type_of(decorator: ast.expr) -> str | None:
    """The kind a bare case-level decorator names, or `None` when the decorator is not one."""
    if isinstance(decorator, ast.Name):
        return CASE_TYPE_DECORATORS.get(decorator.id)
    if isinstance(decorator, ast.Attribute):
        return CASE_TYPE_DECORATORS.get(decorator.attr)
    return None


def _read_case_type(node: ast.FunctionDef | ast.AsyncFunctionDef) -> tuple[str | None, str | None]:
    """The one kind a case declares for every scenario it proves, and a problem when it declares two."""
    labels = [label for decorator in node.decorator_list if (label := _case_type_of(decorator)) is not None]
    if len(set(labels)) > 1:
        return None, f"declares more than one case-level type: {', '.join(sorted(set(labels)))}"
    return (labels[0] if labels else None), None


def _is_scenario_decorator(decorator: ast.expr) -> bool:
    """Whether a decorator is the `scenario` entry point, bare or qualified."""
    if not isinstance(decorator, ast.Call):
        return False
    function = decorator.func
    return (isinstance(function, ast.Name) and function.id == "scenario") or (
        isinstance(function, ast.Attribute) and function.attr == "scenario"
    )


def _read_scenario_call(decorator: ast.Call) -> tuple[str | None, str | None, str | None]:
    """Read one `@scenario` call into its identifier, type label and a problem when malformed."""
    if not decorator.args:
        return None, None, "scenario decorator carries no string identifier"
    if not isinstance(decorator.args[0], ast.Constant) or not isinstance(decorator.args[0].value, str):
        return None, None, "scenario decorator carries no string identifier"
    identifier = decorator.args[0].value
    if not SCENARIO_IDENTIFIER.fullmatch(identifier):
        return None, None, f"{identifier!r} is not a well-formed scenario identifier"
    if decorator.keywords:
        return None, None, "scenario decorator takes the identifier and the type positionally"
    if len(decorator.args) > 2:
        return None, None, "scenario decorator takes one identifier and one optional type"
    if len(decorator.args) == 1:
        return identifier, None, None
    kind = decorator.args[1]
    if not isinstance(kind, ast.Attribute) or kind.attr not in TYPE_LABELS:
        return None, None, "scenario type is not a member of the closed TestType set"
    return identifier, TYPE_LABELS[kind.attr], None


def parse_api_claims(root: Path) -> tuple[dict[str, str], list[str]]:
    """Read the scenarios the fixture fuzz run proves, with the kind of proof each carries."""
    path = root / API_MANIFEST
    if not path.exists():
        return {}, []

    try:
        document = tomllib.loads(path.read_text(encoding="utf-8"))
    except tomllib.TOMLDecodeError as error:
        return {}, [f"{path}: {error}"]

    claims: dict[str, str] = {}
    problems: list[str] = []
    scenarios = document.get("scenarios", {})
    if not isinstance(scenarios, dict):
        return {}, [f"{path}: `scenarios` is not a table"]
    for identifier, kind in scenarios.items():
        if not SCENARIO_IDENTIFIER.fullmatch(identifier):
            problems.append(f"{path}: {identifier!r} is not a well-formed scenario identifier")
            continue
        if kind not in TYPES:
            problems.append(f"{path}: {identifier}: {kind!r} is not one of: {', '.join(TYPES)}")
            continue
        claims[identifier] = kind
    return claims, problems


def parse_rust_claims(root: Path) -> tuple[list[RustClaim], list[str]]:
    """Read every `// proves:` comment in the Rust tree and the function it introduces."""
    claims: list[RustClaim] = []
    problems: list[str] = []

    for path in sorted((root / "src").rglob("*.rs")):
        lines = path.read_text(encoding="utf-8").splitlines()
        for index, line in enumerate(lines):
            proves = RUST_PROVES.match(line)
            if proves is None:
                continue
            body = proves.group("body").strip()
            identifiers, _, note = body.partition(" — ")
            kind, kind_problem = _read_rust_kind(note)
            if kind_problem is not None:
                problems.append(f"{path}:{index + 1}: {kind_problem}")
                continue
            cited = [identifier.strip() for identifier in identifiers.split(",") if identifier.strip()]
            if not cited:
                problems.append(f"{path}:{index + 1}: `// proves:` cites no scenario")
                continue
            function = _next_function(lines, index + 1)
            if function is None:
                problems.append(f"{path}:{index + 1}: `// proves:` is not followed by a function")
                continue
            location = f"{path.relative_to(root).as_posix()}::{function}"
            for identifier in cited:
                if not SCENARIO_IDENTIFIER.fullmatch(identifier):
                    problems.append(f"{path}:{index + 1}: {identifier!r} is not a well-formed scenario identifier")
                    continue
                claims.append(RustClaim(identifier, location, kind))

    return claims, problems


def _read_rust_kind(note: str) -> tuple[str | None, str | None]:
    """The kind a `// proves:` comment declares after its em dash, and a problem when it names no type."""
    note = note.strip()
    if not note:
        return None, None
    label = RUST_KINDS.get(note)
    if label is None:
        return None, f"`// proves:` declares {note!r}, not one of: {', '.join(TYPES)}"
    return label, None


def _next_function(lines: list[str], start: int) -> str | None:
    """The name of the first function declared at or after a line."""
    for line in lines[start:]:
        function = RUST_FUNCTION.match(line)
        if function is not None:
            return function.group("name")
    return None


def load_authored(path: Path) -> Authored:
    """Read and validate the authored annotation sidecar."""
    if not path.exists():
        raise ValueError(f"{path}: the authored annotations file is missing")
    try:
        document = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=_no_duplicate_keys)
    except json.JSONDecodeError as error:
        raise ValueError(f"{path}: {error}") from error
    except ValueError as error:
        raise ValueError(f"{path}: {error}") from error
    if not isinstance(document, dict):
        raise TypeError(f"{path}: the document is not a JSON object")
    unknown = sorted(set(document) - {"causes", "not_black_box", "notes", "not_applicable"})
    if unknown:
        raise ValueError(f"{path}: unknown key{'s' if len(unknown) != 1 else ''}: {', '.join(unknown)}")
    return Authored(
        causes=_read_annotations(path, document.get("causes", {}), "causes"),
        not_black_box=_read_annotations(path, document.get("not_black_box", {}), "not_black_box"),
        notes=_read_annotations(path, document.get("notes", {}), "notes"),
        not_applicable=_read_not_applicable(path, document.get("not_applicable", {})),
    )


def _no_duplicate_keys(pairs: list[tuple[str, object]]) -> dict[str, object]:
    """Build an object, rejecting a duplicate key instead of keeping the last value."""
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate key {key!r}")
        result[key] = value
    return result


def _read_annotations(path: Path, value: object, name: str) -> dict[str, str]:
    """Read one identifier-to-text map from the authored document."""
    if not isinstance(value, dict):
        raise TypeError(f"{path}: `{name}` is not an object")
    annotations: dict[str, str] = {}
    for identifier, text in value.items():
        if not SCENARIO_IDENTIFIER.fullmatch(identifier):
            raise ValueError(f"{path}: {name}: {identifier!r} is not a well-formed scenario identifier")
        if not isinstance(text, str) or not text.strip():
            raise ValueError(f"{path}: {name}: {identifier} carries no text")
        annotations[identifier] = text.strip()
    return annotations


def _read_not_applicable(path: Path, value: object) -> dict[str, list[str]]:
    """Read the requirement-to-type-labels map from the authored document."""
    if not isinstance(value, dict):
        raise TypeError(f"{path}: `not_applicable` is not an object")
    excluded: dict[str, list[str]] = {}
    for identifier, labels in value.items():
        if not REQUIREMENT_IDENTIFIER.fullmatch(identifier):
            raise ValueError(f"{path}: not_applicable: {identifier!r} is not a well-formed requirement identifier")
        if not isinstance(labels, list) or not all(isinstance(label, str) for label in labels):
            raise TypeError(f"{path}: not_applicable: {identifier} is not a list of type labels")
        unknown = sorted(set(labels) - set(TYPES))
        if unknown:
            raise ValueError(f"{path}: not_applicable: {identifier} names unknown types: {', '.join(unknown)}")
        if len(labels) != len(set(labels)):
            raise ValueError(f"{path}: not_applicable: {identifier} repeats a type")
        excluded[identifier] = list(labels)
    return excluded


def build_capabilities(root: Path) -> list[str]:
    """Every capability the specification tree declares, alphabetically."""
    return sorted(path.parent.name.upper() for path in (root / "openspec" / "specs").glob("*/spec.md"))


def render_markdown(text: str) -> Markup:
    """Render the specification's Markdown, or authored text, into escaped HTML."""
    if not text.strip():
        return Markup("")
    return Markup(markdown_lib.markdown(text, extensions=["tables"]))


def _donut_svg(chart_id: str, title: str, description: str, segments: list[dict], total: int) -> Markup:
    """Render one doughnut chart as inline SVG."""
    size = 180
    thickness = 28
    radius = (size - thickness) / 2
    center = size / 2
    parts = [
        (
            f'<svg class="donut" viewBox="0 0 {size} {size}" role="img" '
            f'aria-labelledby="{chart_id}-title {chart_id}-desc">'
        ),
        f'<title id="{chart_id}-title">{html.escape(title)}</title>',
        f'<desc id="{chart_id}-desc">{html.escape(description)}</desc>',
    ]
    if total == 0:
        parts.append(
            f'<circle cx="{center:.2f}" cy="{center:.2f}" r="{radius:.2f}" fill="none" '
            'stroke="currentColor" stroke-opacity="0.18" stroke-width="28"/>'
        )
    else:
        angle = -math.pi / 2
        for segment in segments:
            value = segment["value"]
            if value <= 0:
                continue
            sweep = 2 * math.pi * value / total
            if value == total:
                parts.append(
                    f'<circle cx="{center:.2f}" cy="{center:.2f}" r="{radius:.2f}" fill="none" '
                    f'stroke="{segment["color"]}" stroke-width="{thickness}"/>'
                )
            else:
                start_x = center + radius * math.cos(angle)
                start_y = center + radius * math.sin(angle)
                end_x = center + radius * math.cos(angle + sweep)
                end_y = center + radius * math.sin(angle + sweep)
                large_arc = 1 if sweep > math.pi else 0
                parts.append(
                    f'<path d="M {start_x:.2f} {start_y:.2f} '
                    f'A {radius:.2f} {radius:.2f} 0 {large_arc} 1 {end_x:.2f} {end_y:.2f}" '
                    f'fill="none" stroke="{segment["color"]}" stroke-width="{thickness}" stroke-linecap="butt"/>'
                )
            angle += sweep
    parts.append(
        f'<text x="{center:.2f}" y="{center:.2f}" text-anchor="middle" dominant-baseline="central" '
        f'class="donut-total">{total}</text>'
    )
    parts.append("</svg>")
    return Markup("".join(parts))


def _donut(
    chart_id: str,
    title: str,
    description: str,
    entries: list[tuple[str, int, str]],
) -> dict:
    """Build one doughnut chart's SVG and legend data."""
    total = sum(value for _, value, _ in entries)
    segments = [
        {
            "label": label,
            "value": value,
            "color": color,
            "percent": _percent_text(value, total),
            "width": 0.0 if total == 0 else 100 * value / total,
        }
        for label, value, color in entries
    ]
    return {
        "title": title,
        "description": description,
        "total": total,
        "segments": segments,
        "svg": _donut_svg(chart_id, title, description, segments, total),
    }


def _source_segments(counts: dict[str, int]) -> list[dict]:
    """Build the proof-source distribution of a set of scenarios, each counted once."""
    total = sum(counts.values())
    entries = (
        ("system", SYSTEM_SUITE, counts["system"], COLORS["system"]),
        ("api", API_SUITE, counts["api"], COLORS["api"]),
        ("rust", "Rust", counts["rust"], COLORS["rust"]),
        ("skip", "Skipped", counts["skip"], COLORS[SKIPPED]),
        ("none", "Uncovered", counts["none"], COLORS["none"]),
    )
    return [
        {
            "key": key,
            "label": label,
            "value": value,
            "color": color,
            "percent": _percent_text(value, total),
            "width": 0.0 if total == 0 else 100 * value / total,
        }
        for key, label, value, color in entries
    ]


def build_report(matrix: Matrix) -> tuple[dict, list[str], list[str], int]:
    """Compute every derived view, chart and warning of one generation."""
    requirements = matrix.requirements
    scenarios = matrix.scenarios
    authored = matrix.authored

    claims_by_scenario: dict[str, list[Claim]] = {}
    for claim in matrix.claims:
        claims_by_scenario.setdefault(claim.scenario, []).append(claim)
    rust_proven = {claim.scenario for claim in matrix.rust_claims}
    rust_kinds: dict[str, set[str]] = {}
    for claim in matrix.rust_claims:
        if claim.kind is not None:
            rust_kinds.setdefault(claim.scenario, set()).add(claim.kind)

    def kinds_of(identifier: str) -> set[str]:
        kinds = {claim.kind for claim in claims_by_scenario.get(identifier, []) if claim.kind is not None}
        kinds |= rust_kinds.get(identifier, set())
        api_kind = matrix.api_claims.get(identifier)
        if api_kind is not None:
            kinds.add(api_kind)
        return kinds

    def evidence_of(identifier: str) -> list[str]:
        evidence = []
        if claims_by_scenario.get(identifier):
            evidence.append(SYSTEM_SUITE)
        if matrix.api_claims.get(identifier) is not None:
            evidence.append(API_SUITE)
        if identifier in rust_proven:
            evidence.append(RUST_PROOF)
        return evidence

    scenario_data: dict[str, dict] = {}
    for identifier in scenarios:
        evidence = evidence_of(identifier)
        has_system = SYSTEM_SUITE in evidence
        has_api = API_SUITE in evidence
        has_rust = RUST_PROOF in evidence
        suite_claimed = has_system or has_api
        covered = has_system or has_api or has_rust
        cause = authored.causes.get(identifier, "")
        not_black_box = authored.not_black_box.get(identifier, "")
        note = authored.notes.get(identifier, "")
        if covered:
            status = COVERED
        elif cause:
            status = SKIPPED
        else:
            status = GAP
        if not evidence:
            evidence_key = "none"
        elif len(evidence) > 1:
            evidence_key = "multiple"
        elif has_system:
            evidence_key = "system"
        elif has_api:
            evidence_key = "api"
        else:
            evidence_key = "rust"
        if has_system:
            source = "system"
        elif has_api:
            source = "api"
        elif has_rust:
            source = "rust"
        elif cause:
            source = "skip"
        else:
            source = "none"
        scenario_data[identifier] = {
            "kinds": kinds_of(identifier),
            "evidence": evidence,
            "has_system": has_system,
            "has_api": has_api,
            "has_rust": has_rust,
            "suite_claimed": suite_claimed,
            "covered": covered,
            "source": source,
            "cause": cause,
            "not_black_box": not_black_box,
            "note": note,
            "status": status,
            "evidence_key": evidence_key,
        }

    scenario_status_counts = {status: 0 for status in SCENARIO_STATUS}
    evidence_counts = {"system": 0, "api": 0, "rust": 0, "multiple": 0, "none": 0}
    source_counts = {"system": 0, "api": 0, "rust": 0, "skip": 0, "none": 0}
    type_counts = {label: 0 for label in TYPES}
    suite_claimed_total = 0
    covered_total = 0
    unspecified_total = 0
    for data in scenario_data.values():
        scenario_status_counts[data["status"]] += 1
        evidence_counts[data["evidence_key"]] += 1
        source_counts[data["source"]] += 1
        for kind in data["kinds"]:
            type_counts[kind] += 1
        if data["suite_claimed"]:
            suite_claimed_total += 1
        if data["covered"]:
            covered_total += 1
        if data["suite_claimed"] and not data["kinds"]:
            unspecified_total += 1

    rust_only_total = sum(1 for data in scenario_data.values() if data["has_rust"] and not data["suite_claimed"])
    no_proof_total = len(scenarios) - covered_total
    requirement_status_counts = {status: 0 for status in REQUIREMENT_STATUS}
    contradictions: list[str] = []
    capabilities: list[dict] = []

    for capability in matrix.capabilities:
        declared = sorted(
            (requirement for requirement in requirements.values() if requirement.capability == capability),
            key=lambda requirement: numeric_suffix(requirement.identifier),
        )
        requirement_views: list[dict] = []
        capability_scenarios = 0
        capability_covered = 0
        capability_skipped = 0
        capability_uncovered = 0
        capability_system_api = 0
        capability_rust_only = 0
        capability_unspecified = 0

        for requirement in declared:
            counts = {label: 0 for label in TYPES}
            requirement_covered = 0
            requirement_system_api = 0
            requirement_unspecified = 0
            requirement_sources = {"system": 0, "api": 0, "rust": 0, "skip": 0, "none": 0}
            scenario_views: list[dict] = []

            for scenario in sorted(requirement.scenarios, key=lambda item: scenario_suffix(item.identifier)):
                data = scenario_data[scenario.identifier]
                if data["covered"]:
                    requirement_covered += 1
                elif data["status"] == SKIPPED:
                    capability_skipped += 1
                elif data["status"] == GAP:
                    capability_uncovered += 1
                if data["suite_claimed"]:
                    requirement_system_api += 1
                if data["suite_claimed"] and not data["kinds"]:
                    requirement_unspecified += 1
                requirement_sources[data["source"]] += 1
                for kind in data["kinds"]:
                    counts[kind] += 1
                scenario_views.append(
                    {
                        "identifier": scenario.identifier,
                        "title": scenario.title,
                        "steps": render_markdown(scenario.steps),
                        "kinds": [label for label in TYPES if label in data["kinds"]],
                        "evidence": data["evidence"],
                        "has_system": data["has_system"],
                        "has_api": data["has_api"],
                        "has_rust": data["has_rust"],
                        "proof": data["covered"],
                        "suite_claimed": data["suite_claimed"],
                        "status": data["status"],
                        "has_cause": bool(data["cause"]),
                        "cause": render_markdown(data["cause"]),
                        "has_not_black_box": bool(data["not_black_box"]),
                        "not_black_box": render_markdown(data["not_black_box"]),
                        "has_note": bool(data["note"]),
                        "note": render_markdown(data["note"]),
                        "search": _search_text(
                            scenario.identifier,
                            scenario.title,
                            scenario.steps,
                            data["cause"],
                            data["not_black_box"],
                            data["note"],
                            " ".join(sorted(data["kinds"])),
                            " ".join(data["evidence"]),
                            capability,
                        ),
                    }
                )

            type_cells = []
            excluded = set(authored.not_applicable.get(requirement.identifier, []))
            for label in TYPES:
                if label in excluded:
                    type_cells.append({"label": label, "value": "N/A", "na": True})
                    if counts[label] > 0:
                        proofs = f"{counts[label]} proof{' exists' if counts[label] == 1 else 's exist'}"
                        contradictions.append(f"`{requirement.identifier}`: {label} marked `N/A` but {proofs}")
                else:
                    type_cells.append({"label": label, "value": str(counts[label]), "na": False})

            status = _status(requirement_covered, len(requirement.scenarios))
            requirement_status_counts[status] += 1
            declared_count = len(requirement.scenarios)
            covered_pct = _percent(requirement_covered, declared_count)
            requirement_views.append(
                {
                    "identifier": requirement.identifier,
                    "title": requirement.title,
                    "description": render_markdown(requirement.description),
                    "status": status,
                    "capability": capability,
                    "scenario_count": declared_count,
                    "covered": requirement_covered,
                    "covered_pct": covered_pct,
                    "uncovered_pct": 100 - covered_pct,
                    "system_api": requirement_system_api,
                    "unspecified": requirement_unspecified,
                    "sources": _source_segments(requirement_sources),
                    "types": type_cells,
                    "scenarios": scenario_views,
                    "search": _search_text(
                        requirement.identifier,
                        requirement.title,
                        requirement.description,
                        capability,
                    ),
                }
            )

            capability_scenarios += declared_count
            capability_covered += requirement_covered
            capability_system_api += requirement_system_api
            capability_rust_only += sum(
                1
                for scenario in requirement.scenarios
                if scenario_data[scenario.identifier]["has_rust"]
                and not scenario_data[scenario.identifier]["suite_claimed"]
            )
            capability_unspecified += requirement_unspecified

        covered_pct = _percent(capability_covered, capability_scenarios)
        skipped_pct = _percent(capability_skipped, capability_scenarios)
        uncovered_pct = 100 - covered_pct - skipped_pct
        open_requirements = covered_pct < 100
        for requirement in requirement_views:
            requirement["open"] = open_requirements
        capabilities.append(
            {
                "name": capability,
                "display": display_name(capability),
                "anchor": "cap-" + re.sub(r"[^a-z0-9]+", "-", capability.lower()).strip("-"),
                "requirements": requirement_views,
                "requirement_count": len(requirement_views),
                "scenario_count": capability_scenarios,
                "covered": capability_covered,
                "covered_pct": covered_pct,
                "skipped": capability_skipped,
                "skipped_pct": skipped_pct,
                "uncovered": capability_uncovered,
                "uncovered_pct": uncovered_pct,
                "system_api": capability_system_api,
                "rust_only": capability_rust_only,
                "unspecified": capability_unspecified,
            }
        )

    stale: list[str] = []
    for identifier in sorted(authored.causes):
        if identifier not in scenarios:
            stale.append(f"`{identifier}` has a Cause but no declared scenario.")
    for identifier in sorted(authored.not_black_box):
        if identifier not in scenarios:
            stale.append(f"`{identifier}` has a Why-not-black-box memo but no declared scenario.")
    for identifier in sorted(authored.notes):
        if identifier not in scenarios:
            stale.append(f"`{identifier}` has a Note but no declared scenario.")
    for identifier in sorted(authored.not_applicable):
        if identifier not in requirements:
            stale.append(f"`{identifier}` marks N/A types but no declared requirement.")

    total_scenarios = len(scenarios)
    covered_pct = _percent(covered_total, total_scenarios)
    system_api_pct = _percent(suite_claimed_total, total_scenarios)

    kpis = [
        {"label": "Requirements", "value": str(len(requirements)), "detail": "declared"},
        {"label": "Scenarios", "value": str(total_scenarios), "detail": "declared"},
        {
            "label": "Covered",
            "value": f"{covered_pct}%",
            "detail": f"{covered_total} of {total_scenarios}",
        },
        {
            "label": "Requirements tested",
            "value": str(requirement_status_counts[TESTED]),
            "detail": (
                f"{requirement_status_counts[PARTIAL]} partial · {requirement_status_counts[NOT_TESTED]} not tested"
            ),
        },
        {
            "label": "No proof",
            "value": str(no_proof_total),
            "detail": f"{scenario_status_counts[SKIPPED]} skipped · {scenario_status_counts[GAP]} uncovered",
        },
        {
            "label": "Uncovered",
            "value": str(scenario_status_counts[GAP]),
            "detail": "no proof and no Cause",
        },
        {"label": "Rust-only", "value": str(rust_only_total), "detail": "proven by Rust alone"},
        {
            "label": "System/API",
            "value": str(suite_claimed_total),
            "detail": f"{system_api_pct}% of scenarios",
        },
        {"label": "API-suite", "value": str(len(matrix.api_claims)), "detail": "fuzz-run claims"},
        {"label": "Untyped", "value": str(unspecified_total), "detail": "system/API claim, no type"},
    ]

    scenario_pie = _donut(
        "scenario-pie",
        "Scenario coverage",
        "Every declared scenario, coloured by whether it is covered, skipped or a gap.",
        [
            ("Covered", scenario_status_counts[COVERED], COLORS[COVERED]),
            ("Skipped", scenario_status_counts[SKIPPED], COLORS[SKIPPED]),
            ("Uncovered", scenario_status_counts[GAP], COLORS[GAP]),
        ],
    )
    requirement_pie = _donut(
        "requirement-pie",
        "Requirement status",
        "Requirements by whether every, some or none of their scenarios is covered.",
        [
            (TESTED, requirement_status_counts[TESTED], COLORS[TESTED]),
            (PARTIAL, requirement_status_counts[PARTIAL], COLORS[PARTIAL]),
            (NOT_TESTED, requirement_status_counts[NOT_TESTED], COLORS[NOT_TESTED]),
        ],
    )
    evidence_pie = _donut(
        "evidence-pie",
        "Evidence mix",
        "Scenarios by the evidence that proves them.",
        [
            ("System only", evidence_counts["system"], COLORS["system"]),
            ("API only", evidence_counts["api"], COLORS["api"]),
            ("Rust only", evidence_counts["rust"], COLORS["rust"]),
            ("More than one", evidence_counts["multiple"], COLORS["multiple"]),
            ("None", evidence_counts["none"], COLORS["none"]),
        ],
    )
    source_split = _source_segments(source_counts)
    max_type = max(type_counts.values(), default=0) or 1
    type_bars = [
        {"label": label, "count": type_counts[label], "pct": round(100 * type_counts[label] / max_type)}
        for label in TYPES
    ]

    report = {
        "kpis": kpis,
        "scenario_pie": scenario_pie,
        "requirement_pie": requirement_pie,
        "evidence_pie": evidence_pie,
        "source_split": source_split,
        "capability_bars": sorted(capabilities, key=lambda item: (item["covered_pct"], item["name"])),
        "type_bars": type_bars,
        "contradictions": contradictions,
        "stale": stale,
        "uncovered": scenario_status_counts[GAP],
        "capabilities": capabilities,
        "total_requirements": len(requirements),
        "total_scenarios": total_scenarios,
        "authored_href": AUTHORED.name,
    }
    return report, contradictions, stale, scenario_status_counts[GAP]


def _status(tested: int, declared: int) -> str:
    """A requirement's status from how many of its scenarios carry a proof."""
    if declared == 0 or tested == 0:
        return NOT_TESTED
    if tested == declared:
        return TESTED
    return PARTIAL


def render_document(report: dict) -> str:
    """Render the report through its Jinja2 template."""
    environment = Environment(
        loader=FileSystemLoader(TEMPLATE_DIR),
        autoescape=select_autoescape(("html", "jinja")),
        trim_blocks=True,
        lstrip_blocks=True,
        keep_trailing_newline=True,
    )
    environment.filters["markdown"] = render_markdown
    template = environment.get_template(TEMPLATE_NAME)
    return template.render(report=report).rstrip("\n") + "\n"


def main(root: Path, *, check: bool = False) -> int:
    """Generate the report and return the process exit code."""
    try:
        requirements, scenarios, problems = parse_specifications(root)
    except ValueError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    if problems:
        for problem in problems:
            print(f"error: {problem}", file=sys.stderr)
        return 2

    claims, claim_problems = parse_system_claims(root)
    api_claims, api_problems = parse_api_claims(root)
    rust_claims, rust_problems = parse_rust_claims(root)
    problems = claim_problems + api_problems + rust_problems

    known = set(scenarios)
    for claim in claims:
        if claim.scenario not in known:
            problems.append(f"{claim.test}: claims {claim.scenario}, which no specification declares")
    for identifier in api_claims:
        if identifier not in known:
            problems.append(f"{API_MANIFEST}: {identifier} is not declared by any specification")
    for claim in rust_claims:
        if claim.scenario not in known:
            problems.append(f"{claim.location}: proves {claim.scenario}, which no specification declares")
    if problems:
        for problem in problems:
            print(f"error: {problem}", file=sys.stderr)
        return 2

    try:
        authored = load_authored(root / AUTHORED)
    except (TypeError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2

    matrix = Matrix(
        requirements=requirements,
        scenarios=scenarios,
        claims=claims,
        api_claims=api_claims,
        rust_claims=rust_claims,
        capabilities=build_capabilities(root),
        authored=authored,
    )
    report, contradictions, stale, uncovered = build_report(matrix)
    text = render_document(report)
    artefact = root / ARTEFACT

    if check:
        if not artefact.exists():
            print(f"error: {ARTEFACT} does not exist; run script/investigation.sh", file=sys.stderr)
            return 1
        if artefact.read_text(encoding="utf-8") != text:
            print(f"error: {ARTEFACT} is stale; run script/investigation.sh", file=sys.stderr)
            return 1
    else:
        artefact.parent.mkdir(parents=True, exist_ok=True)
        artefact.write_text(text, encoding="utf-8")

    for contradiction in contradictions:
        print(f"warning: {contradiction}", file=sys.stderr)
    for item in stale:
        print(f"warning: stale annotation: {item}", file=sys.stderr)
    if uncovered:
        print(
            f"warning: {uncovered} uncovered scenario{'s' if uncovered != 1 else ''} carry no proof and no Cause; "
            "they are the worklist",
            file=sys.stderr,
        )
    return 0


if __name__ == "__main__":
    arguments = sys.argv[1:]
    checking = "--check" in arguments
    if checking:
        arguments.remove("--check")
    if len(arguments) != 1:
        print("usage: python3 -m investigation <repository-root> [--check]", file=sys.stderr)
        raise SystemExit(2)
    raise SystemExit(main(Path(arguments[0]), check=checking))
