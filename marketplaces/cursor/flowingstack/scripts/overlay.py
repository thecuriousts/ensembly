import os


def merge_rows(base, extra, key):
    order, by = [], {}
    for row in base:
        k = row[key]
        if k not in by:
            order.append(k)
        by[k] = row
    for row in extra:
        k = row[key]
        if k not in by:
            order.append(k)
        by[k] = row
    return [by[k] for k in order]


def user_path(env_name, local):
    pointed = os.environ.get(env_name)
    if pointed:
        return pointed if os.path.isfile(pointed) else None
    if local and os.path.isfile(local):
        return local
    return None
