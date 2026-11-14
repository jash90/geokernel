#!/usr/bin/env python3
"""Extract t("input", "expected", ...) pairs from GeoGebra's kernel tests.

Usage:
    python3 -I tools/extract-t-pairs/extract_t_pairs.py \
        --source <geogebra checkout> --out golden/t-pairs

Only the Python standard library is used. Nothing is guessed: a call whose
arguments are not all string literals (after resolving constants read from
GeoGebra's own Unicode.java) is written to skipped.jsonl with a reason.
GeoGebra sources are licensed under EUPL 1.2 (copyright GeoGebra GmbH).
"""
import argparse
import json
import os
import re
import subprocess
import sys
from collections import Counter, OrderedDict

PINNED_COMMIT = "112919f2d450ec7430a5cd3b65dba226fdc43b74"
DEFAULT_SCOPE = "source/shared/common-jre/src/test/java/org/geogebra/common/kernel"
UNICODE_FILE = "Unicode.java"
EUPL_MARKER = "under the EUPL 1.2 licence"
OTHER_HELPER_RE = re.compile(r"^t(?:[A-Z]\w*|[a-z]{1,2})$")
JAVA_KEYWORDS = {"try", "throw", "this"}
NOT_A_TYPE = {"return", "new", "else", "throw", "case", "assert", "yield",
              "instanceof", "do"}


# ---------------------------------------------------------------- lexer

class Tok:
    __slots__ = ("kind", "val", "start", "end", "line")

    def __init__(self, kind, val, start, end, line):
        self.kind, self.val, self.start, self.end, self.line = (
            kind, val, start, end, line)


def lex(text):
    """Tokenize Java; comments are dropped. Kinds: id str chr tblock num op."""
    toks, i, n, line = [], 0, len(text), 1
    while i < n:
        c = text[i]
        if c == "\n":
            line += 1
            i += 1
        elif c in " \t\r\f":
            i += 1
        elif text.startswith("//", i):
            j = text.find("\n", i)
            i = n if j < 0 else j
        elif text.startswith("/*", i):
            j = text.find("*/", i + 2)
            j = n if j < 0 else j + 2
            line += text.count("\n", i, j)
            i = j
        elif text.startswith('"""', i):
            j = i + 3
            while j < n and not (text.startswith('"""', j)
                                 and text[j - 1] != "\\"):
                j += 1
            j = min(n, j + 3)
            toks.append(Tok("tblock", text[i:j], i, j, line))
            line += text.count("\n", i, j)
            i = j
        elif c in "\"'":
            j = i + 1
            while j < n and text[j] != c and text[j] != "\n":
                j += 2 if text[j] == "\\" else 1
            j = min(n, j + 1)
            kind = "str" if c == '"' else "chr"
            toks.append(Tok(kind, text[i + 1:j - 1], i, j, line))
            i = j
        elif c.isalpha() or c in "_$":
            j = i + 1
            while j < n and (text[j].isalnum() or text[j] in "_$"):
                j += 1
            toks.append(Tok("id", text[i:j], i, j, line))
            i = j
        elif c.isdigit():
            j = i + 1
            while j < n and (text[j].isalnum() or text[j] in "_."):
                j += 1
            toks.append(Tok("num", text[i:j], i, j, line))
            i = j
        else:
            w = 2 if text.startswith(("::", "->"), i) else 1
            toks.append(Tok("op", text[i:i + w], i, i + w, line))
            i += w
    return toks


ESCAPE_RE = re.compile(r"\\(u+[0-9a-fA-F]{4}|[0-3][0-7]{0,2}|[4-7][0-7]?|.)",
                       re.S)
SIMPLE_ESCAPES = {"n": "\n", "t": "\t", "r": "\r", "b": "\b", "f": "\f",
                  "s": " ", '"': '"', "'": "'", "\\": "\\"}


def unescape(raw):
    """Decode a Java string/char literal body. Raises ValueError if invalid."""
    def rep(m):
        e = m.group(1)
        if e[0] == "u":
            return chr(int(e.lstrip("u"), 16))
        if e[0].isdigit():
            return chr(int(e, 8))
        if e in SIMPLE_ESCAPES:
            return SIMPLE_ESCAPES[e]
        raise ValueError("unsupported escape \\" + e)
    out = ESCAPE_RE.sub(rep, raw)
    try:  # joins surrogate pairs; lone surrogates are rejected
        return out.encode("utf-16", "surrogatepass").decode("utf-16")
    except UnicodeDecodeError:
        raise ValueError("lone surrogate in literal")


# ------------------------------------------------------ constant resolution

def match_paren(toks, k):
    """Index of the ')' (or closer) matching the opener at toks[k]."""
    depth = 0
    for j in range(k, len(toks)):
        v = toks[j].val if toks[j].kind == "op" else None
        if v in ("(", "[", "{"):
            depth += 1
        elif v in (")", "]", "}"):
            depth -= 1
            if depth == 0:
                return j
    return -1


def load_unicode_constants(source_root):
    """Read `static final char|String NAME = ...;` from Unicode.java."""
    found = []
    for dp, _dn, fns in os.walk(os.path.join(source_root, "source")):
        if UNICODE_FILE in fns:
            found.append(os.path.join(dp, UNICODE_FILE))
    if len(found) != 1:
        raise SystemExit("expected exactly one %s, found %d"
                         % (UNICODE_FILE, len(found)))
    path = found[0]
    with open(path, encoding="utf-8") as fh:
        text = fh.read()
    if EUPL_MARKER not in text[:2000]:
        raise SystemExit("%s has no EUPL 1.2 header; refusing to use it"
                         % UNICODE_FILE)
    toks = lex(text)
    pending, table = {}, {}
    for k, t in enumerate(toks):
        if (t.kind == "id" and t.val in ("char", "String") and k + 3 < len(toks)
                and toks[k + 1].kind == "id" and toks[k + 2].val == "="
                and k > 0 and toks[k - 1].val == "final"):
            end = k + 3
            while end < len(toks) and toks[end].val != ";":
                end += 1
            pending[toks[k + 1].val] = (t.val, toks[k + 3:end], t.line)
    for _ in range(10):  # fixpoint over constants defined via other constants
        for name, (typ, expr, line) in pending.items():
            if name in table:
                continue
            val = eval_constant(typ, expr, table)
            if val is not None:
                table[name] = (typ, val, line)
    rel = os.path.relpath(path, source_root).replace(os.sep, "/")
    return table, rel


def eval_constant(typ, expr, table):
    try:
        if len(expr) == 1 and expr[0].kind in ("str", "chr"):
            return unescape(expr[0].val)
        if (len(expr) == 6 and expr[0].val in ("Character", "String")
                and expr[2].val in ("toString", "valueOf")
                and expr[4].kind == "id" and expr[4].val in table):
            return table[expr[4].val][1]
        if len(expr) >= 3 and all(
                (e.kind in ("str", "chr")) if i % 2 == 0 else e.val == "+"
                for i, e in enumerate(expr)) and typ == "String":
            return "".join(unescape(e.val) for e in expr[::2])
    except ValueError:
        return None
    return None


# ------------------------------------------------------------- extraction

class Skip(Exception):
    pass


def eval_arg(toks, constants, used):
    """Evaluate one argument made of string literals and Unicode constants."""
    if not toks:
        raise Skip("empty argument")
    terms, cur = [], []
    for t in toks:
        if t.kind == "op" and t.val == "+":
            terms.append(cur)
            cur = []
        else:
            cur.append(t)
    terms.append(cur)
    acc, acc_type = "", None
    for term in terms:
        if len(term) == 1 and term[0].kind == "str":
            typ, val = "str", unescape(term[0].val)
        elif len(term) == 1 and term[0].kind == "chr":
            typ, val = "char", unescape(term[0].val)
        elif len(term) == 3 and term[0].val == "Unicode" and term[1].val == ".":
            name = term[2].val
            if name not in constants:
                raise Skip("unresolved constant: Unicode." + name)
            ctype, val, _ = constants[name]
            typ = "char" if ctype == "char" else "str"
            used[name] += 1
        elif len(term) == 1 and term[0].kind == "tblock":
            raise Skip("text block")
        else:
            text = " ".join(t.val for t in term)
            if "StringTemplate" in text or text.split(".")[0] == "tpl":
                raise Skip("template argument: " + text)
            raise Skip("non-literal argument: " + text)
        if acc_type is None:
            acc, acc_type = val, typ
        elif acc_type == "char" and typ == "char":
            raise Skip("char arithmetic (char + char is an int in Java)")
        else:
            acc, acc_type = acc + val, "str"
    return acc


def split_args(toks, open_idx, close_idx):
    args, cur, depth = [], [], 0
    for t in toks[open_idx + 1:close_idx]:
        if t.kind == "op" and t.val in "([{":
            depth += 1
        elif t.kind == "op" and t.val in ")]}":
            depth -= 1
        if t.kind == "op" and t.val == "," and depth == 0:
            args.append(cur)
            cur = []
        else:
            cur.append(t)
    if cur or args:
        args.append(cur)
    return args


def enclosing_methods(toks):
    """For every token index, the name of the innermost enclosing method."""
    method_at = {}
    for k, t in enumerate(toks):
        if (t.kind == "id" and k > 0 and k + 1 < len(toks)
                and toks[k + 1].val == "(" and toks[k + 1].kind == "op"
                and ((toks[k - 1].kind == "id"
                      and toks[k - 1].val not in NOT_A_TYPE)
                     or toks[k - 1].val in (">", "]"))):
            j = match_paren(toks, k + 1)
            if j < 0:
                continue
            j += 1
            while j < len(toks) and (toks[j].kind == "id"
                                     or toks[j].val in (",", ".")):
                j += 1
            if j < len(toks) and toks[j].val == "{":
                method_at[j] = t.val
    stack, result = [], []
    for k, t in enumerate(toks):
        if t.kind == "op" and t.val == "{":
            stack.append(method_at.get(k))
        elif t.kind == "op" and t.val == "}" and stack:
            stack.pop()
        name = next((m for m in reversed(stack) if m), None)
        result.append(name)
    return result


def has_one_param_helper(toks):
    """True if the file declares its own `t(Type name)` with one parameter."""
    for k in range(1, len(toks) - 4):
        if (toks[k].val == "t" and toks[k].kind == "id"
                and toks[k - 1].kind == "id"
                and toks[k - 1].val not in NOT_A_TYPE
                and toks[k + 1].val == "(" and toks[k + 2].kind == "id"
                and toks[k + 3].kind == "id" and toks[k + 4].val == ")"):
            return True
    return False


def extract_file(text, rel, constants, used, other):
    """Return (pairs, skipped, found, declarations) for one file."""
    toks = lex(text)
    methods = enclosing_methods(toks)
    pairs, skipped, found, decls = [], [], 0, 0
    one_param_helper = has_one_param_helper(toks)
    for k, t in enumerate(toks):
        if (t.kind != "id" or k + 1 >= len(toks) or toks[k + 1].val != "("
                or toks[k + 1].kind != "op"):
            continue
        prev = toks[k - 1] if k else None
        if prev is not None and prev.val in (".", "::") and prev.kind == "op":
            continue
        is_decl = (prev is not None and prev.kind == "id"
                   and prev.val not in NOT_A_TYPE)
        if t.val != "t":
            if (OTHER_HELPER_RE.match(t.val) and not is_decl
                    and t.val not in JAVA_KEYWORDS):
                other[t.val] += 1
            continue
        if is_decl:
            decls += 1
            continue
        found += 1
        close = match_paren(toks, k + 1)
        if close < 0:
            skipped.append(skip_row(rel, t, "unbalanced parentheses",
                                    text[t.start:t.start + 80]))
            continue
        source = text[t.start:toks[close].end]
        args = split_args(toks, k + 1, close)
        local_used = Counter()
        try:
            if not args:
                raise Skip("no arguments")
            if len(args) == 1 and one_param_helper:
                raise Skip("file-local helper t(String) takes no expected "
                           "value")
            vals = [eval_arg(a, constants, local_used) for a in args]
        except Skip as e:
            skipped.append(skip_row(rel, t, str(e), source))
            continue
        except ValueError as e:
            skipped.append(skip_row(rel, t, "bad literal: " + str(e), source))
            continue
        used.update(local_used)
        pairs.append(OrderedDict([("file", rel), ("line", t.line),
                                  ("test", methods[k]), ("input", vals[0]),
                                  ("expected", vals[1:])]))
    return pairs, skipped, found, decls


def skip_row(rel, t, reason, source):
    return OrderedDict([("file", rel), ("line", t.line), ("reason", reason),
                        ("source", source)])


# ------------------------------------------------------------------ output

def dump_jsonl(path, rows):
    with open(path, "w", encoding="utf-8", newline="\n") as fh:
        for r in rows:
            fh.write(json.dumps(r, ensure_ascii=True) + "\n")


def category(reason):
    return reason.split(":")[0]


def write_report(path, stats):
    L = []
    a = L.append
    a("# t-pairs extraction report\n")
    a("Generated by `tools/extract-t-pairs/extract_t_pairs.py`. "
      "Do not edit by hand.\n")
    a("GeoGebra commit: `%s`\n" % stats["commit"])
    a("## Denominator: plain grep\n")
    a("Scope: `%s` (recursive, `*.java`).\n" % stats["scope"])
    a("```\ngrep -rE '^\\s*t\\(\"' <geogebra>/%s --include='*.java' | wc -l\n```"
      % stats["scope"])
    a("\nResult: **%d** lines (plan figure: 1762, %s).\n"
      % (stats["grep_linestart"],
         "reproduced exactly" if stats["grep_linestart"] == 1762
         else "NOT reproduced"))
    a("Other counts in the same scope, for context:\n")
    a("- lines containing a `t(` token (any position, includes declarations "
      "and comments): %d" % stats["grep_anywhere"])
    a("- calls found by the lexer (comments excluded, declarations "
      "excluded): %d" % stats["found"])
    a("- `t(` helper declarations: %d\n" % stats["decls"])
    a("The grep counts lines that start with `t(\"`. It misses calls that "
      "start with `t(` on its own line, with a non-literal first argument, "
      "or inside another expression, so it differs from the lexer count.\n")
    a("## Totals\n")
    a("- calls found: %d" % stats["found"])
    a("- extracted: %d" % stats["extracted"])
    a("- skipped: %d\n" % stats["skipped"])
    a("### Skipped by reason\n")
    a("| reason | count |\n|---|---|")
    for k, v in sorted(stats["reasons"].items(), key=lambda kv: (-kv[1], kv[0])):
        a("| %s | %d |" % (k, v))
    a("\n### Other helpers (not matched as `t`)\n")
    if stats["other"]:
        a("| helper | calls |\n|---|---|")
        for k, v in sorted(stats["other"].items(),
                           key=lambda kv: (-kv[1], kv[0])):
            a("| %s | %d |" % (k, v))
    else:
        a("none")
    a("\n### Constants resolved from `%s`\n" % stats["unicode_file"])
    a("The defining file carries the EUPL 1.2 header. Values were read from "
      "its literals; calls using them were extracted.\n")
    if stats["used"]:
        a("| constant | uses in extracted calls |\n|---|---|")
        for k, v in sorted(stats["used"].items(),
                           key=lambda kv: (-kv[1], kv[0])):
            a("| Unicode.%s | %d |" % (k, v))
    else:
        a("none")
    a("\n## Per file\n")
    a("| file | found | extracted | skipped |\n|---|---|---|---|")
    for f, (fo, ex, sk) in sorted(stats["per_file"].items()):
        a("| %s | %d | %d | %d |" % (f, fo, ex, sk))
    with open(path, "w", encoding="utf-8", newline="\n") as fh:
        fh.write("\n".join(L) + "\n")


def git_head(root):
    try:
        out = subprocess.run(["git", "-C", root, "rev-parse", "HEAD"],
                             capture_output=True, text=True, check=True)
        return out.stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return None


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--source", required=True, help="GeoGebra checkout")
    ap.add_argument("--out", required=True, help="output directory")
    ap.add_argument("--scope", default=DEFAULT_SCOPE,
                    help="test directory relative to --source")
    ap.add_argument("--allow-other-commit", action="store_true")
    args = ap.parse_args(argv)

    head = git_head(args.source)
    if head != PINNED_COMMIT and not args.allow_other_commit:
        sys.exit("GeoGebra checkout is at %s, expected %s (use "
                 "--allow-other-commit to override)" % (head, PINNED_COMMIT))
    constants, unicode_rel = load_unicode_constants(args.source)
    scope_dir = os.path.join(args.source, args.scope)
    files = []
    for dp, dn, fns in os.walk(scope_dir):
        dn.sort()
        files += [os.path.join(dp, f) for f in fns if f.endswith(".java")]
    files.sort(key=lambda p: os.path.relpath(p, args.source).replace(os.sep, "/"))

    all_pairs, all_skipped, per_file = [], [], {}
    used, other = Counter(), Counter()
    found = decls = grep_start = grep_any = 0
    start_re = re.compile(r'^\s*t\("')
    any_re = re.compile(r"(^|[^A-Za-z0-9_.])t\(")
    for p in files:
        rel = os.path.relpath(p, args.source).replace(os.sep, "/")
        with open(p, encoding="utf-8", newline="") as fh:
            text = fh.read()
        for ln in text.split("\n"):
            grep_start += bool(start_re.match(ln))
            grep_any += bool(any_re.search(ln))
        pairs, skipped, fo, dc = extract_file(text, rel, constants, used, other)
        found += fo
        decls += dc
        if fo:
            per_file[rel] = (fo, len(pairs), len(skipped))
        all_pairs += pairs
        all_skipped += skipped

    os.makedirs(args.out, exist_ok=True)
    dump_jsonl(os.path.join(args.out, "t-pairs.jsonl"), all_pairs)
    dump_jsonl(os.path.join(args.out, "skipped.jsonl"), all_skipped)
    stats = {
        "commit": head or "unknown", "scope": args.scope,
        "grep_linestart": grep_start, "grep_anywhere": grep_any,
        "found": found, "decls": decls, "extracted": len(all_pairs),
        "skipped": len(all_skipped),
        "reasons": Counter(category(s["reason"]) for s in all_skipped),
        "other": other, "used": used, "unicode_file": unicode_rel,
        "per_file": per_file,
    }
    write_report(os.path.join(args.out, "REPORT.md"), stats)
    print("found=%d extracted=%d skipped=%d grep_linestart=%d"
          % (found, len(all_pairs), len(all_skipped), grep_start))


if __name__ == "__main__":
    main()
