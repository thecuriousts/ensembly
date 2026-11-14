"""mitmdump addon: one JSON line per flow. Metadata only: never headers, bodies, or query strings."""
import json, os, time
from mitmproxy import http

OUT = os.environ["FLOWLOG_OUT"]
PHASE_FILE = os.environ["FLOWLOG_PHASE"]

def phase():
    try:
        return open(PHASE_FILE).read().strip()
    except OSError:
        return "?"

def owner(port):
    """Find the local process that owns 127.0.0.1:<port> (client side of the proxy connection)."""
    inode = None
    try:
        for line in open("/proc/net/tcp").readlines()[1:]:
            f = line.split()
            lp = int(f[1].split(":")[1], 16)
            rp = int(f[2].split(":")[1], 16)
            if lp == port and rp == int(os.environ.get("FLOWLOG_PROXY_PORT", "8899")):
                inode = f[9]
                break
    except OSError:
        return None
    if not inode:
        return None
    target = f"socket:[{inode}]"
    for pid in os.listdir("/proc"):
        if not pid.isdigit():
            continue
        try:
            for fd in os.listdir(f"/proc/{pid}/fd"):
                if os.readlink(f"/proc/{pid}/fd/{fd}") == target:
                    comm = open(f"/proc/{pid}/comm").read().strip()
                    exe = os.path.basename(os.readlink(f"/proc/{pid}/exe"))
                    return {"pid": int(pid), "comm": comm, "exe": exe}
        except OSError:
            continue
    return None

OWNERS = {}

def client_connected(client):
    port = client.peername[1]
    OWNERS[port] = owner(port)

def _write(rec):
    with open(OUT, "a") as fh:
        fh.write(json.dumps(rec) + "\n")

def http_connect(flow: http.HTTPFlow):
    _write({"t": time.strftime("%H:%M:%S"), "phase": phase(), "kind": "CONNECT",
            "host": flow.request.host, "port": flow.request.port,
            "proc": OWNERS.get(flow.client_conn.peername[1])})

def response(flow: http.HTTPFlow):
    r = flow.request
    _write({"t": time.strftime("%H:%M:%S"), "phase": phase(), "kind": "HTTP",
            "scheme": r.scheme, "host": r.pretty_host, "port": r.port, "method": r.method,
            "path": r.path.split("?")[0][:200], "status": flow.response.status_code,
            "req_bytes": len(r.raw_content or b""), "resp_bytes": int(flow.response.headers.get("content-length") or len(flow.response.raw_content or b"")),
            "proc": OWNERS.get(flow.client_conn.peername[1])})

def error(flow):
    r = getattr(flow, "request", None)
    _write({"t": time.strftime("%H:%M:%S"), "phase": phase(), "kind": "ERROR",
            "host": r.pretty_host if r else None, "port": r.port if r else None,
            "err": str(flow.error)[:200], "proc": OWNERS.get(flow.client_conn.peername[1])})
