#!/usr/bin/env python3
import json
import subprocess
from concurrent.futures import ThreadPoolExecutor, as_completed
from urllib.parse import quote_plus

from metrics import cited_prs, is_public, take_public

DEFAULT_OWNERS = ("p10ns11y", "thecuriousts")


class GhError(RuntimeError):
    pass


def gh_json(args):
    proc = subprocess.run(["gh", "api", *args], capture_output=True, text=True)
    if proc.returncode != 0:
        detail = (proc.stderr or proc.stdout or "gh api failed").strip().splitlines()
        raise GhError(detail[-1] if detail else "gh api failed")
    if not proc.stdout.strip():
        return None
    return json.loads(proc.stdout)


def list_public_repos(owners=None):
    repos = []
    seen = set()
    for owner in owners or DEFAULT_OWNERS:
        proc = subprocess.run(
            [
                "gh", "repo", "list", owner,
                "--limit", "1000",
                "--visibility", "public",
                "--json", "nameWithOwner,isPrivate,visibility",
            ],
            capture_output=True,
            text=True,
        )
        if proc.returncode != 0:
            raise GhError((proc.stderr or proc.stdout or "gh repo list failed").strip().splitlines()[-1])
        for repo in json.loads(proc.stdout or "[]"):
            item = {
                "full_name": repo.get("nameWithOwner"),
                "visibility": repo.get("visibility") or "",
                "private": bool(repo.get("isPrivate")),
            }
            if not item["full_name"] or item["full_name"] in seen:
                continue
            seen.add(item["full_name"])
            repos.append(item)
    return take_public(repos)


def search_pages(path, query):
    page = 1
    items = []
    while page <= 10:
        data = gh_json([
            "-H", "Accept: application/vnd.github+json",
            f"{path}?q={quote_plus(query)}&per_page=100&page={page}",
        ])
        batch = (data or {}).get("items") or []
        items.extend(batch)
        total = (data or {}).get("total_count") or 0
        if not batch or len(items) >= total or len(batch) < 100:
            break
        page += 1
    return items


def repo_from_url(url):
    marker = "/repos/"
    if not url or marker not in url:
        return None
    return url.split(marker, 1)[1].strip("/")


PR_QUERY = """
query($owner: String!, $name: String!, $number: Int!) {
  repository(owner: $owner, name: $name) {
    visibility
    isPrivate
    pullRequest(number: $number) {
      number
      title
      body
      createdAt
      mergedAt
      labels(first: 30) { nodes { name } }
      commits(first: 100) {
        nodes {
          commit {
            oid
            checkSuites(first: 20) {
              nodes {
                checkRuns(first: 40) {
                  nodes { name conclusion startedAt }
                }
              }
            }
          }
        }
      }
    }
  }
}
"""


def flatten_runs(pr):
    first_head = None
    runs = []
    for node in ((pr.get("commits") or {}).get("nodes") or []):
        commit = (node or {}).get("commit") or {}
        sha = commit.get("oid")
        found = []
        for suite in ((commit.get("checkSuites") or {}).get("nodes") or []):
            for run in ((suite.get("checkRuns") or {}).get("nodes") or []):
                found.append({
                    "name": run.get("name"),
                    "head_sha": sha,
                    "started_at": run.get("startedAt"),
                    "conclusion": run.get("conclusion"),
                })
        if found:
            first_head = sha
            runs = found
            break
    return first_head, runs


def load_pull(full_name, number):
    owner, name = full_name.split("/", 1)
    data = gh_json([
        "graphql",
        "-f", f"query={PR_QUERY}",
        "-F", f"owner={owner}",
        "-F", f"name={name}",
        "-F", f"number={int(number)}",
    ])
    repo = ((data or {}).get("data") or {}).get("repository") or {}
    vis = str(repo.get("visibility") or "").lower()
    probe = {"full_name": full_name, "visibility": vis, "private": bool(repo.get("isPrivate"))}
    if vis == "":
        probe["visibility"] = "public" if repo.get("isPrivate") is False else ""
    if not is_public(probe):
        return None
    pr = repo.get("pullRequest")
    if not pr or not pr.get("mergedAt"):
        return None
    first_head, runs = flatten_runs(pr)
    return {
        "repo": full_name,
        "number": pr.get("number") or number,
        "title": pr.get("title") or "",
        "body": pr.get("body") or "",
        "labels": [node.get("name") for node in ((pr.get("labels") or {}).get("nodes") or []) if node.get("name")],
        "created_at": pr.get("createdAt"),
        "merged_at": pr.get("mergedAt"),
        "first_head_sha": first_head,
        "check_runs": runs,
    }


def load_cited(full_name, number):
    try:
        data = gh_json([
            "-H", "Accept: application/vnd.github+json",
            f"/repos/{full_name}/pulls/{int(number)}",
        ])
    except GhError:
        return None
    repo_info = data.get("base", {}).get("repo") or data.get("head", {}).get("repo") or {}
    vis = str(repo_info.get("visibility") or "").lower()
    if not vis and repo_info.get("private") is False:
        vis = "public"
    probe = {
        "full_name": repo_info.get("full_name") or full_name,
        "visibility": vis,
        "private": bool(repo_info.get("private")),
    }
    if not is_public(probe):
        return None
    return {
        "repo": probe["full_name"],
        "number": data.get("number") or number,
        "title": data.get("title") or "",
        "body": data.get("body") or "",
        "labels": [label.get("name") for label in data.get("labels") or [] if label.get("name")],
        "created_at": data.get("created_at"),
        "merged_at": data.get("merged_at"),
        "check_runs": [],
    }


def collect_snapshot(since, until, owners=None):
    owners = list(owners or DEFAULT_OWNERS)
    public_repos = list_public_repos(owners)
    public = {repo["full_name"] for repo in public_repos}
    seen = set()
    found = []
    for owner in owners:
        for prefix in (f"user:{owner}", f"org:{owner}"):
            query = f"{prefix} is:pr is:merged merged:{since.isoformat()}..{until.isoformat()}"
            for item in search_pages("/search/issues", query):
                full_name = repo_from_url(item.get("repository_url") or "")
                if not full_name or full_name not in public:
                    continue
                key = (full_name, item.get("number"))
                if key in seen:
                    continue
                seen.add(key)
                found.append(key)

    pulls = []
    with ThreadPoolExecutor(max_workers=6) as pool:
        futures = [pool.submit(load_pull, repo, number) for repo, number in found]
        for fut in as_completed(futures):
            loaded = fut.result()
            if loaded:
                pulls.append(loaded)
    pulls.sort(key=lambda item: (item["repo"], item["number"]))

    have = {(item["repo"], item["number"]) for item in pulls}
    extra = set()
    for pull in pulls:
        for cited in cited_prs(pull.get("title"), pull.get("body"), pull.get("repo")):
            if cited[0] in public and cited not in have:
                extra.add(cited)
    with ThreadPoolExecutor(max_workers=6) as pool:
        futures = [pool.submit(load_cited, repo, number) for repo, number in extra]
        for fut in as_completed(futures):
            loaded = fut.result()
            if not loaded:
                continue
            key = (loaded["repo"], loaded["number"])
            if key in have:
                continue
            pulls.append(loaded)
            have.add(key)

    commits = []
    seen_sha = set()
    for owner in owners:
        for prefix in (f"user:{owner}", f"org:{owner}"):
            query = f"{prefix} committer-date:{since.isoformat()}..{until.isoformat()} revert"
            for item in search_pages("/search/commits", query):
                repo = item.get("repository") or {}
                full_name = repo.get("full_name")
                if not full_name or full_name not in public or repo.get("private") is True:
                    continue
                sha = item.get("sha")
                if not sha or sha in seen_sha:
                    continue
                seen_sha.add(sha)
                commits.append({
                    "repo": full_name,
                    "sha": sha,
                    "message": ((item.get("commit") or {}).get("message") or ""),
                    "committed_at": ((item.get("commit") or {}).get("committer") or {}).get("date"),
                })

    return {"repos": public_repos, "pulls": pulls, "commits": commits}
