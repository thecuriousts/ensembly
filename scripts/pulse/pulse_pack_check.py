#!/usr/bin/env python3
"""Canonical pulse pack (pulse-pack-v1, canon rule 1): check, canonicalize, read a store.

Stdlib only, one file, so it runs under `python3 -I -X utf8`. The Rust twin is
`ensembly_kernel::pulse_pack::canonical`; both must reproduce the golden vectors in
crates/ensembly-kernel/tests/golden/pulse-pack/ byte for byte. Spec:
docs/spec/PULSE-PACK-SPEC.md.

  canon <pack>                                    canonical bytes on stdout, exit 0
  canon --db P --exported-at T --host H           same, built from a store opened read-only
  check <pack>                                    verdict bytes on stdout, exit 0/1/2
  ro-uri <path>                                   the read-only SQLite URI for a path

Invalid input: nothing on stdout, verdict bytes on stderr, exit 2.
"""
import argparse
import hashlib
import json
import os
import sqlite3
import sys
import urllib.parse

CANON_FORMAT = "pulse-pack-v1"
CANON_VERSION = 1
RUST_FORMATS = ("ensembly-pulse-pack-v1", "peram-pulse-pack-v1")
MAX_SAFE_INT = 9007199254740991
HOST_ALIASES = ("laptop-1", "laptop-2", "mac-mini", "box")
STATUSES = ("user-stated", "tool-verified", "inferred", "unknown")
KINDS = ("data", "context", "fact", "memory")
LOCKS = ("open", "locked")
TRACE_FIELDS = (
    "nat_key", "snippet", "source", "time", "status", "kind", "lock", "verify_hash",
    "prefetch_key", "seen_count", "superseded_by", "first_seen", "last_seen",
)
NULLABLE = ("verify_hash", "prefetch_key", "superseded_by")
TIME_FIELDS = ("time", "first_seen", "last_seen")


class Float:
    """A JSON number that is not a plain integer (fraction, exponent, or -0)."""

    def __init__(self, text):
        self.text = text

    def value(self):
        return float(self.text)


class StrictJsonError(Exception):
    pass


# ------------------------------------------------------------------ JSON in/out

def _pairs(pairs):
    out = {}
    for k, v in pairs:
        if k in out:
            raise StrictJsonError("duplicate key")
        out[k] = v
    return out


def _const(name):
    raise StrictJsonError("non-finite number " + name)


def _int(text):
    return Float(text) if text == "-0" else int(text)


def _check_strings(v):
    if isinstance(v, str):
        v.encode("utf-8")  # a lone surrogate raises
    elif isinstance(v, list):
        for x in v:
            _check_strings(x)
    elif isinstance(v, dict):
        for k, x in v.items():
            k.encode("utf-8")
            _check_strings(x)


def parse_strict(raw):
    """bytes -> value. Raises StrictJsonError on bad UTF-8, BOM, NaN, dup keys, surrogates."""
    if raw.startswith(b"\xef\xbb\xbf"):
        raise StrictJsonError("BOM")
    try:
        text = raw.decode("utf-8")
        v = json.loads(text, object_pairs_hook=_pairs, parse_constant=_const,
                       parse_float=Float, parse_int=_int)
        _check_strings(v)
    except StrictJsonError:
        raise
    except (ValueError, UnicodeError, RecursionError) as e:
        raise StrictJsonError(str(e))
    return v


def canonical_json(v):
    return json.dumps(v, ensure_ascii=False, sort_keys=True, separators=(",", ":"),
                      allow_nan=False)


def canonical_bytes(v):
    return (canonical_json(v) + "\n").encode("utf-8")


def sha256hex(b):
    return hashlib.sha256(b).hexdigest()


def content_hash(snippet):
    return "sha256:" + sha256hex(snippet.encode("utf-8"))


def pack_sha256(doc):
    return sha256hex(canonical_bytes(doc))


def content_sha256(doc):
    return sha256hex(canonical_bytes({"archive": doc["archive"], "traces": doc["traces"]}))


# ------------------------------------------------------------------ timestamps

DIGITS = "0123456789"


def _num(s):
    if not s or any(c not in DIGITS for c in s):
        return None
    return int(s)


def _days_in_month(y, m):
    if m == 2:
        return 29 if (y % 4 == 0 and (y % 100 != 0 or y % 400 == 0)) else 28
    return 30 if m in (4, 6, 9, 11) else 31


def _days_from_civil(y, m, d):
    y -= m <= 2
    era = y // 400
    yoe = y - era * 400
    doy = (153 * (m + (-3 if m > 2 else 9)) + 2) // 5 + d - 1
    doe = yoe * 365 + yoe // 4 - yoe // 100 + doy
    return era * 146097 + doe - 719468


def _civil_from_days(z):
    z += 719468
    era = z // 146097
    doe = z - era * 146097
    yoe = (doe - doe // 1460 + doe // 36524 - doe // 146096) // 365
    y = yoe + era * 400
    doy = doe - (365 * yoe + yoe // 4 - yoe // 100)
    mp = (5 * doy + 2) // 153
    d = doy - (153 * mp + 2) // 5 + 1
    m = mp + (3 if mp < 10 else -9)
    return y + (m <= 2), m, d


def canonical_ts(s):
    """Strict timestamp -> 'YYYY-MM-DDTHH:MM:SS[.frac]Z' in UTC, or None."""
    if not isinstance(s, str):
        return None
    date, rest = s[:10], s[10:]
    if len(date) != 10 or date[4] != "-" or date[7] != "-":
        return None
    y, mo, d = _num(date[0:4]), _num(date[5:7]), _num(date[8:10])
    if y is None or mo is None or d is None:
        return None
    hh = mi = ss = 0
    frac = ""
    off = 0
    if rest == "":
        pass
    elif len(rest) == 6 and rest[0] == " " and rest[3] == ":":
        hh, mi = _num(rest[1:3]), _num(rest[4:6])
        if hh is None or mi is None:
            return None
    elif len(rest) >= 10 and rest[0] == "T" and rest[3] == ":" and rest[6] == ":":
        hh, mi, ss = _num(rest[1:3]), _num(rest[4:6]), _num(rest[7:9])
        if hh is None or mi is None or ss is None:
            return None
        tail = rest[9:]
        if tail.startswith("."):
            i = 1
            while i < len(tail) and tail[i] in DIGITS:
                i += 1
            frac = tail[1:i]
            if not 1 <= len(frac) <= 9:
                return None
            tail = tail[i:]
        if tail == "Z":
            off = 0
        elif len(tail) == 6 and tail[0] in "+-" and tail[3] == ":":
            oh, om = _num(tail[1:3]), _num(tail[4:6])
            if oh is None or om is None or oh > 23 or om > 59:
                return None
            off = (oh * 60 + om) * (1 if tail[0] == "+" else -1)
        else:
            return None
    else:
        return None
    if not (1 <= mo <= 12 and 1 <= d <= _days_in_month(y, mo)):
        return None
    if hh > 23 or mi > 59 or ss > 59:
        return None
    secs = _days_from_civil(y, mo, d) * 86400 + hh * 3600 + mi * 60 + ss - off * 60
    days, sod = divmod(secs, 86400)
    y2, m2, d2 = _civil_from_days(days)
    if not 1 <= y2 <= 9999:
        return None
    frac = frac.rstrip("0")
    out = "%04d-%02d-%02dT%02d:%02d:%02d" % (y2, m2, d2, sod // 3600, sod // 60 % 60, sod % 60)
    return out + ("." + frac if frac else "") + "Z"


# ------------------------------------------------------------------ validation

def _ptr(*parts):
    return "".join("/" + str(p).replace("~", "~0").replace("/", "~1") for p in parts)


def _has_float(v):
    if isinstance(v, Float):
        return True
    if isinstance(v, list):
        return any(_has_float(x) for x in v)
    if isinstance(v, dict):
        return any(_has_float(x) for x in v.values())
    return False


def _big_int(v):
    if type(v) is int:
        return abs(v) > MAX_SAFE_INT
    if isinstance(v, list):
        return any(_big_int(x) for x in v)
    if isinstance(v, dict):
        return any(_big_int(x) for x in v.values())
    return False


class Ctx:
    def __init__(self):
        self.errors = []

    def err(self, code, path):
        self.errors.append((path, code))

    def field(self, obj, key, path, kind, required=True, nullable=False):
        """Missing -> type -> float -> range. Returns (ok, value)."""
        p = path + _ptr(key)
        if key not in obj:
            if required:
                self.err("E_MISSING", p)
                return False, None
            return True, None
        v = obj[key]
        if v is None and nullable:
            return True, None
        if kind == "str":
            if not isinstance(v, str):
                self.err("E_TYPE", p)
                return False, None
        elif kind == "int":
            if isinstance(v, Float):
                self.err("E_FLOAT", p)
                return False, None
            if type(v) is not int:
                self.err("E_TYPE", p)
                return False, None
            if abs(v) > MAX_SAFE_INT:
                self.err("E_RANGE", p)
                return False, None
        elif kind == "num":
            if not (isinstance(v, Float) or type(v) is int):
                self.err("E_TYPE", p)
                return False, None
        elif kind == "obj":
            if not isinstance(v, dict):
                self.err("E_TYPE", p)
                return False, None
        elif kind == "list":
            if not isinstance(v, list):
                self.err("E_TYPE", p)
                return False, None
        elif kind == "any":
            if _has_float(v):
                self.err("E_FLOAT", p)
                return False, None
            if _big_int(v):
                self.err("E_RANGE", p)
                return False, None
        return True, v

    def unknown(self, obj, allowed, path):
        for k in obj:
            if k not in allowed:
                self.err("E_UNKNOWN_FIELD", path + _ptr(k))

    def time(self, obj, key, path, required=True):
        ok, v = self.field(obj, key, path, "str", required)
        if not ok or v is None:
            return None
        t = canonical_ts(v)
        if t is None:
            self.err("E_TIME", path + _ptr(key))
        return t

    def enum(self, obj, key, path, allowed, code):
        ok, v = self.field(obj, key, path, "str")
        if ok and v not in allowed:
            self.err(code, path + _ptr(key))
            return None
        return v if ok else None


def _ext_trace(c, ext, path):
    out = {}
    c.unknown(ext, ("coherence_milli", "content", "kind", "metadata"), path)
    ok, v = c.field(ext, "coherence_milli", path, "int", required=False)
    if ok and v is not None:
        if not 0 <= v <= 1000:
            c.err("E_RANGE", path + _ptr("coherence_milli"))
        out["coherence_milli"] = v
    ok, v = c.field(ext, "content", path, "any", required=False)
    if ok and "content" in ext:
        out["content"] = v
    ok, v = c.field(ext, "kind", path, "str", required=False)
    if ok and v is not None:
        out["kind"] = v
    ok, v = c.field(ext, "metadata", path, "obj", required=False)
    if ok and v is not None:
        if c.field(ext, "metadata", path, "any")[0]:
            out["metadata"] = v
    return out


def _python_trace(c, t, path):
    if not isinstance(t, dict):
        c.err("E_TYPE", path)
        return None
    c.unknown(t, TRACE_FIELDS + ("ext",), path)
    out = {}
    ok, v = c.field(t, "nat_key", path, "str")
    if ok and v == "":
        c.err("E_RANGE", path + _ptr("nat_key"))
    out["nat_key"] = v
    for k in ("snippet", "source"):
        out[k] = c.field(t, k, path, "str")[1]
    for k in TIME_FIELDS:
        out[k] = c.time(t, k, path)
    out["status"] = c.enum(t, "status", path, STATUSES, "E_STATUS")
    out["kind"] = c.enum(t, "kind", path, KINDS, "E_KIND")
    out["lock"] = c.enum(t, "lock", path, LOCKS, "E_LOCK")
    for k in NULLABLE:
        ok, v = c.field(t, k, path, "str", required=False, nullable=True)
        out[k] = v if ok and v != "" else None
    ok, v = c.field(t, "seen_count", path, "int")
    if ok and v < 1:
        c.err("E_RANGE", path + _ptr("seen_count"))
    out["seen_count"] = v
    if "ext" in t:
        ok, ext = c.field(t, "ext", path, "obj")
        if ok:
            e = _ext_trace(c, ext, path + _ptr("ext"))
            if e:
                out["ext"] = e
    return out


def _python_event(c, e, path):
    if not isinstance(e, dict):
        c.err("E_TYPE", path)
        return None
    c.unknown(e, ("correlation_id", "payload", "source", "ts", "ext"), path)
    out = {
        "correlation_id": c.field(e, "correlation_id", path, "str", False, True)[1],
        "payload": c.field(e, "payload", path, "str")[1],
        "source": c.field(e, "source", path, "str")[1],
        "ts": c.time(e, "ts", path),
    }
    if "ext" in e:
        ok, ext = c.field(e, "ext", path, "obj")
        if ok:
            c.unknown(ext, ("kind",), path + _ptr("ext"))
            ok, k = c.field(ext, "kind", path + _ptr("ext"), "str", required=False)
            if ok and k is not None:
                out["ext"] = {"kind": k}
    return out


def _rust_trace(c, t, path):
    if not isinstance(t, dict):
        c.err("E_TYPE", path)
        return None
    c.unknown(t, ("nat_key", "ts", "kind", "content", "coherence", "agent_id", "metadata",
                  "admitted_at"), path)
    ok, key = c.field(t, "nat_key", path, "str")
    if ok and key == "":
        c.err("E_RANGE", path + _ptr("nat_key"))
    time = c.time(t, "ts", path)
    admitted = c.time(t, "admitted_at", path)
    ok, kind = c.field(t, "kind", path, "str")
    source = c.field(t, "agent_id", path, "str")[1]
    ext = {"kind": kind}
    ok, content = c.field(t, "content", path, "any")
    snippet = None
    if ok:
        if isinstance(content, str):
            snippet = content
        else:
            snippet = canonical_json(content)
            ext["content"] = content
    ok, coh = c.field(t, "coherence", path, "num")
    if ok:
        x = coh.value() if isinstance(coh, Float) else float(coh)
        milli = int((x * 1000 + 0.5) // 1)
        if not 0 <= milli <= 1000:
            c.err("E_RANGE", path + _ptr("coherence"))
        ext["coherence_milli"] = milli
    if "metadata" in t:
        ok, meta = c.field(t, "metadata", path, "obj")
        if ok and c.field(t, "metadata", path, "any")[0] and meta:
            ext["metadata"] = meta
    return {
        "nat_key": key, "snippet": snippet, "source": source, "time": time,
        "status": "unknown", "kind": "memory", "lock": "open", "verify_hash": None,
        "prefetch_key": None, "seen_count": 1, "superseded_by": None,
        "first_seen": admitted, "last_seen": admitted, "ext": ext,
    }


def _rust_event(c, e, path):
    if not isinstance(e, dict):
        c.err("E_TYPE", path)
        return None
    c.unknown(e, ("nat_key", "ts", "kind", "payload"), path)
    key = c.field(e, "nat_key", path, "str")[1]
    ts = c.time(e, "ts", path)
    kind = c.field(e, "kind", path, "str")[1]
    ok, payload = c.field(e, "payload", path, "any")
    if "payload" not in e:
        ok = False
    return {"correlation_id": key, "payload": canonical_json(payload) if ok else None,
            "source": "ensembly", "ts": ts, "ext": {"kind": kind}}


def analyze(v):
    """value -> dict(doc, errors, warnings, traces, archive, source_format)."""
    c = Ctx()
    res = {"doc": None, "errors": [], "warnings": [], "traces": 0, "archive": 0,
           "source_format": None}
    if not isinstance(v, dict):
        res["errors"] = [("", "E_FORMAT")] if False else [("/format", "E_FORMAT")]
        return res
    fmt = v.get("format")
    res["source_format"] = fmt if isinstance(fmt, str) else None
    rust = fmt in RUST_FORMATS
    if fmt != CANON_FORMAT and not rust:
        res["errors"] = [("/format", "E_FORMAT")]
        return res
    tkey, akey, hkey = ("memory_traces", "archive_events", "source_host") if rust else \
        ("traces", "archive", "host")
    for key, slot in ((tkey, "traces"), (akey, "archive")):
        if isinstance(v.get(key), list):
            res[slot] = len(v[key])
    doc = {"format": CANON_FORMAT}
    if rust:
        c.unknown(v, ("format", "exported_at", "source_host", "pack_hash", "memory_hash",
                      "memory_traces", "archive_events", "memory_crdt"), "")
        ok, ph = c.field(v, "pack_hash", "", "str")
        ok2, mh = c.field(v, "memory_hash", "", "str", required=False, nullable=True)
        doc["ext"] = {"memory_hash": mh if ok2 else None, "source_format": fmt,
                      "source_pack_hash": ph}
    else:
        c.unknown(v, ("format", "exported_at", "host", "traces", "archive", "ext"), "")
        if "ext" in v:
            ok, ext = c.field(v, "ext", "", "obj")
            if ok:
                c.unknown(ext, ("memory_hash", "source_format", "source_pack_hash"), "/ext")
                e = {}
                ok, x = c.field(ext, "memory_hash", "/ext", "str", False, True)
                if ok and "memory_hash" in ext:
                    e["memory_hash"] = x
                for k in ("source_format", "source_pack_hash"):
                    ok, x = c.field(ext, k, "/ext", "str", required=False)
                    if ok and x is not None:
                        e[k] = x
                if e:
                    doc["ext"] = e
    doc["exported_at"] = c.time(v, "exported_at", "")
    doc["host"] = c.field(v, hkey, "", "str")[1]
    traces = []
    ok, tl = c.field(v, tkey, "", "list")
    if ok:
        for i, t in enumerate(tl):
            p = _ptr(tkey, i)
            traces.append((rust and _rust_trace or _python_trace)(c, t, p))
    events = []
    if akey in v:
        ok, al = c.field(v, akey, "", "list")
        if ok:
            for i, e in enumerate(al):
                p = _ptr(akey, i)
                events.append((rust and _rust_event or _python_event)(c, e, p))
    # Cross-row rules, on rows that parsed.
    seen = set()
    for i, t in enumerate(traces):
        if t is None:
            continue
        k = t["nat_key"]
        if isinstance(k, str) and k:
            if k in seen:
                c.err("E_DUP_KEY", _ptr(tkey, i, "nat_key"))
            seen.add(k)
        if t["status"] == "tool-verified" and isinstance(t["snippet"], str):
            if t["verify_hash"] != content_hash(t["snippet"]):
                c.err("E_EVIDENCE", _ptr(tkey, i, "verify_hash"))
    if c.errors:
        res["errors"] = sorted(set(c.errors))
        return res
    warnings = []
    if doc["host"] not in HOST_ALIASES:
        warnings.append((_ptr(hkey), "W_HOST_ALIAS"))
    for i, t in enumerate(traces):
        if t["superseded_by"] is not None and t["superseded_by"] not in seen:
            warnings.append((_ptr(tkey, i, "superseded_by"), "W_DANGLING_SUPERSEDED"))
        if (t["status"] != "tool-verified" and t["verify_hash"] is not None
                and t["verify_hash"] != content_hash(t["snippet"])):
            warnings.append((_ptr(tkey, i, "verify_hash"), "W_VERIFY_MISMATCH"))
    for t in traces:
        if "ext" in t and not t["ext"]:
            del t["ext"]
    doc["traces"] = sorted(traces, key=lambda t: t["nat_key"].encode("utf-8"))
    doc["archive"] = sorted(events, key=lambda e: canonical_json(e).encode("utf-8"))
    res["doc"] = doc
    res["warnings"] = sorted(set(warnings))
    return res


def _verdict(res, json_error=False):
    errors = [("", "E_JSON")] if json_error else res["errors"]
    ok = not errors
    doc = res["doc"] if ok else None
    warn = ok and bool(res["warnings"])
    issue = lambda xs: [{"code": c, "path": p} for p, c in xs]
    return {
        "archive": 0 if json_error else res["archive"],
        "canon": CANON_VERSION,
        "content_sha256": content_sha256(doc) if doc else None,
        "errors": issue(errors),
        "exit": 2 if not ok else (1 if warn else 0),
        "format": CANON_FORMAT if ok else None,
        "pack_sha256": pack_sha256(doc) if doc else None,
        "source_format": None if json_error else res["source_format"],
        "traces": 0 if json_error else res["traces"],
        "verdict": "fail" if not ok else ("warn" if warn else "ok"),
        "warnings": issue(res["warnings"]) if ok else [],
    }


def check(raw):
    try:
        v = parse_strict(raw)
    except StrictJsonError:
        return None, _verdict(None, json_error=True)
    res = analyze(v)
    return res["doc"], _verdict(res)


# ------------------------------------------------------------------ store, read-only

def ro_uri(path):
    b = os.fsencode(path)
    return "file:" + urllib.parse.quote(b, safe="/-._~") + "?mode=ro"


def open_ro(path):
    conn = sqlite3.connect(ro_uri(path), uri=True)
    conn.execute("PRAGMA query_only=ON")
    return conn


def from_db(path, exported_at, host):
    conn = open_ro(path)
    try:
        cols = ", ".join(TRACE_FIELDS)
        traces = [dict(zip(TRACE_FIELDS, r)) for r in
                  conn.execute("SELECT %s FROM memory_traces ORDER BY id" % cols)]
        archive = []
        has = conn.execute(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='archive_events'"
        ).fetchone()[0]
        if has:
            ekeys = ("ts", "source", "payload", "correlation_id")
            archive = [dict(zip(ekeys, r)) for r in conn.execute(
                "SELECT ts, source, payload, correlation_id FROM archive_events ORDER BY id")]
    finally:
        conn.close()
    for t in traces:
        for k in NULLABLE:
            if t[k] is None:
                del t[k]
    return {"format": CANON_FORMAT, "exported_at": exported_at, "host": host,
            "traces": traces, "archive": archive}


# ------------------------------------------------------------------ CLI

def _out(b):
    sys.stdout.buffer.write(b)
    sys.stdout.buffer.flush()


def main(argv=None):
    ap = argparse.ArgumentParser(prog="pulse_pack_check.py")
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("canon")
    c.add_argument("pack", nargs="?")
    c.add_argument("--db")
    c.add_argument("--exported-at")
    c.add_argument("--host")
    k = sub.add_parser("check")
    k.add_argument("pack")
    r = sub.add_parser("ro-uri")
    r.add_argument("path")
    a = ap.parse_args(argv)

    if a.cmd == "ro-uri":
        _out((ro_uri(a.path) + "\n").encode("utf-8"))
        return 0
    if a.cmd == "check":
        _, verdict = check(open(a.pack, "rb").read())
        _out(canonical_bytes(verdict))
        return verdict["exit"]
    if a.db is not None:
        if a.pack is not None or a.exported_at is None or a.host is None:
            ap.error("canon --db needs --exported-at and --host and no pack")
        try:
            v = from_db(a.db, a.exported_at, a.host)
        except sqlite3.Error as e:
            sys.stderr.write("pulse store: %s\n" % e)
            return 2
        res = analyze(v)
        doc, verdict = res["doc"], _verdict(res)
    else:
        if a.pack is None:
            ap.error("canon needs a pack or --db")
        doc, verdict = check(open(a.pack, "rb").read())
    if doc is None:
        sys.stderr.buffer.write(canonical_bytes(verdict))
        return 2
    _out(canonical_bytes(doc))
    return 0


if __name__ == "__main__":
    sys.exit(main())
