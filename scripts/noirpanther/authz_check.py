#!/usr/bin/env python3
"""Authorization regression check for a NoirPanther server (run it against a TEST server).

    python3 scripts/noirpanther/authz_check.py http://localhost:10912 \\
        --owner cat:cattest12345 --user dog:dogtest12345 [--outsider panther_m0:memberpass123]

Covers the holes closed in 0.1.7-r4 (all of them upstream Stump behaviour, or upstream-merge
fallout): self-service privilege escalation through updateViewer / updateUser, per-user
content rules not hiding BOOKS, metadata overview leaking hidden values, the unguarded
missing-entities listing and the unguarded previous-discussions query.

It changes the regular user's permissions / age restriction / content rules through the
owner account and restores them at the end. Exit code 1 when any check fails.
"""
import argparse
import http.cookiejar
import json
import sys
import urllib.error
import urllib.request


class Client:
    def __init__(self, base, creds):
        self.base = base.rstrip("/")
        user, _, password = creds.partition(":")
        jar = http.cookiejar.CookieJar()
        self.opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))
        self._post("/api/v2/auth/login", {"username": user, "password": password})

    def _post(self, path, payload):
        req = urllib.request.Request(
            self.base + path,
            data=json.dumps(payload).encode(),
            headers={"Content-Type": "application/json"},
        )
        return self.opener.open(req, timeout=60).read()

    def gql(self, query, variables=None):
        return json.loads(self._post("/api/graphql", {"query": query, "variables": variables or {}}))

    def status(self, path):
        try:
            return self.opener.open(self.base + path, timeout=60).status
        except urllib.error.HTTPError as error:
            return error.code


FAILED = []


def check(name, ok, detail=""):
    print(("ok    " if ok else "FAIL  ") + name + (f"  [{detail}]" if detail and not ok else ""))
    if not ok:
        FAILED.append(name)


UPDATE_VIEWER = "mutation($i: UpdateUserInput!){ updateViewer(input:$i){ permissions maxSessionsAllowed ageRestriction{ age } } }"
UPDATE_USER = "mutation($id: ID!, $i: UpdateUserInput!){ updateUser(id:$id, input:$i){ permissions maxSessionsAllowed ageRestriction{ age } } }"
SET_RULES = "mutation($u: ID!, $r: [ContentAccessRuleInput!]!){ setUserContentAccessRules(userId:$u, rules:$r){ id } }"
ME = "{ me { id username permissions maxSessionsAllowed ageRestriction{ age restrictOnUnset } } }"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("base")
    parser.add_argument("--owner", required=True)
    parser.add_argument("--user", required=True)
    parser.add_argument("--outsider")
    args = parser.parse_args()

    owner = Client(args.base, args.owner)
    user = Client(args.base, args.user)
    me = user.gql(ME)["data"]["me"]
    username = me["username"]
    original = {
        "username": username,
        "permissions": me["permissions"],
        "ageRestriction": me["ageRestriction"],
        "maxSessionsAllowed": me["maxSessionsAllowed"],
    }

    # 1. self-service escalation
    attack = {"username": username, "permissions": ["MANAGE_USERS", "MANAGE_LIBRARY", "DOWNLOAD_FILE"], "ageRestriction": None, "maxSessionsAllowed": 99}
    user.gql(UPDATE_VIEWER, {"i": attack})
    after = Client(args.base, args.user).gql(ME)["data"]["me"]
    check("updateViewer cannot change own permissions", sorted(after["permissions"]) == sorted(me["permissions"]), after["permissions"])
    check("updateViewer cannot change own session limit", after["maxSessionsAllowed"] == me["maxSessionsAllowed"], after["maxSessionsAllowed"])
    user.gql(UPDATE_USER, {"id": me["id"], "i": attack})
    after = Client(args.base, args.user).gql(ME)["data"]["me"]
    check("updateUser(self) cannot change own permissions", sorted(after["permissions"]) == sorted(me["permissions"]), after["permissions"])

    # 2. age restriction set by the owner cannot be lifted by the user
    owner.gql(UPDATE_USER, {"id": me["id"], "i": {**original, "ageRestriction": {"age": 10, "restrictOnUnset": False}}})
    Client(args.base, args.user).gql(UPDATE_VIEWER, {"i": {**original, "ageRestriction": None}})
    after = Client(args.base, args.user).gql(ME)["data"]["me"]
    check("owner can set an age restriction", (after["ageRestriction"] or {}).get("age") == 10, after["ageRestriction"])
    check("user cannot lift own age restriction", after["ageRestriction"] is not None)
    owner.gql(UPDATE_USER, {"id": me["id"], "i": original})

    # 3. the profile form (echoes the current values back) still works
    result = Client(args.base, args.user).gql(UPDATE_VIEWER, {"i": original})
    check("profile form round-trip still succeeds", "errors" not in result, result.get("errors"))

    # 4. content rules hide books everywhere
    overview = owner.gql("{ mediaMetadataOverview{ genres } }")["data"]["mediaMetadataOverview"]["genres"]
    total = owner.gql("{ mediaCount }")["data"]["mediaCount"]
    genre = None
    for candidate in overview:
        hit = owner.gql("query($f: MediaFilterInput!){ media(filter:$f, pagination:{offset:{page:1,pageSize:1}}){ nodes{ id resolvedName } } }", {"f": {"metadata": {"genres": {"contains": candidate}}}})
        nodes = (hit.get("data") or {}).get("media", {}).get("nodes") or []
        if nodes:
            genre, hidden = candidate, nodes[0]
            break
    if genre is None:
        print("skip  content-rule checks (no book with a genre on this server)")
    else:
        owner.gql(SET_RULES, {"u": me["id"], "r": [{"dimension": "GENRE", "mode": "EXCLUDE", "values": [genre], "restrictOnUnset": False}]})
        restricted = Client(args.base, args.user)
        visible = restricted.gql("{ mediaCount }")["data"]["mediaCount"]
        check(f"content rule (exclude genre {genre!r}) lowers the book count", visible < total, f"{visible} of {total}")
        by_id = restricted.gql("query($id: ID!){ mediaById(id:$id){ id } }", {"id": hidden["id"]})
        check("rule-hidden book is not reachable by id", not (by_id.get("data") or {}).get("mediaById"))
        check("rule-hidden book: REST thumbnail refused", restricted.status(f"/api/v2/media/{hidden['id']}/thumbnail") in (403, 404))
        check("rule-hidden book: REST page refused", restricted.status(f"/api/v2/media/{hidden['id']}/page/1") in (403, 404))
        seen = restricted.gql("{ mediaMetadataOverview{ genres } }")["data"]["mediaMetadataOverview"]["genres"]
        check("metadata overview does not list the hidden genre", genre not in seen)
        owner.gql(SET_RULES, {"u": me["id"], "r": []})

    # 5. maintenance listing is for library managers only
    library = owner.gql("{ libraries(pagination:{none:{unpaginated:true}}){ nodes{ id } } }")["data"]["libraries"]["nodes"][0]["id"]
    missing = "query($l: ID!){ libraryMissingEntities(libraryId:$l){ nodes{ path } } }"
    check("libraryMissingEntities refused without MANAGE_LIBRARY", "errors" in Client(args.base, args.user).gql(missing, {"l": library}))
    check("libraryMissingEntities works for the owner", "errors" not in owner.gql(missing, {"l": library}))

    # 6. previous discussions of a club the caller cannot read
    if args.outsider:
        clubs = owner.gql("{ bookClubs(all:true){ id isPrivate } }")["data"]["bookClubs"]
        private = [club for club in clubs if club["isPrivate"]]
        if private:
            outsider = Client(args.base, args.outsider)
            result = outsider.gql("query($c: ID!){ previousBookClubDiscussions(bookClubId:$c){ id } }", {"c": private[0]["id"]})
            check("previousBookClubDiscussions refused for a non-member of a private club", "errors" in result)
        else:
            print("skip  previousBookClubDiscussions (no private club on this server)")

    print(f"\n{'ALL CHECKS PASSED' if not FAILED else 'FAILED: ' + ', '.join(FAILED)}")
    sys.exit(1 if FAILED else 0)


if __name__ == "__main__":
    main()
