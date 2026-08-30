"""One-shot: wipe every tag row across every .magpie DB the user has
touched, reset the automatic-AI-tagging fingerprints, and re-index
FTS so search still works.

Each DB is backed up to `<name>.bak-YYYYMMDD-HHMMSS` before being
touched, so the operation is reversible with a plain file rename.

Usage:
    py scripts/wipe_tags.py <path-to-magpie-1> [<path-to-magpie-2> ...]

The script refuses to run if any target file looks locked (another
process — usually a running Magpie — has it open on Windows).
"""

from __future__ import annotations

import os
import shutil
import sqlite3
import sys
from datetime import datetime


def looks_locked(path: str) -> bool:
    """Best-effort Windows check: try to open the file for exclusive
    write. If another process is holding it (Magpie running), this
    fails with a PermissionError."""
    try:
        with open(path, "r+b"):
            return False
    except PermissionError:
        return True
    except OSError:
        # Missing / corrupt — the caller will surface a better error
        # when it tries to open the DB.
        return False


def wipe(db_path: str) -> None:
    if not os.path.isfile(db_path):
        print(f"MISSING  {db_path}")
        return
    if looks_locked(db_path):
        print(f"LOCKED   {db_path}  (close Magpie and retry)")
        return

    ts = datetime.now().strftime("%Y%m%d-%H%M%S")
    backup = f"{db_path}.bak-{ts}"
    shutil.copy2(db_path, backup)

    con = sqlite3.connect(db_path)
    con.execute("PRAGMA foreign_keys = ON")
    try:
        cur = con.cursor()

        # Column set differs across schema versions (v2 didn't have
        # ai_tagged_at / ai_tag_hash yet). Detect what's actually on
        # the images table so this script works on any DB the user
        # hasn't opened in the newer Magpie yet.
        image_cols = {
            row[1] for row in cur.execute("PRAGMA table_info(images)").fetchall()
        }
        has_ai_cols = {"ai_tagged_at", "ai_tag_hash"}.issubset(image_cols)

        before_image_tags = cur.execute("SELECT COUNT(*) FROM image_tags").fetchone()[0]
        before_tags = cur.execute("SELECT COUNT(*) FROM tags").fetchone()[0]
        before_ai_tagged = (
            cur.execute(
                "SELECT COUNT(*) FROM images WHERE ai_tagged_at IS NOT NULL"
            ).fetchone()[0]
            if has_ai_cols
            else 0
        )
        image_count = cur.execute("SELECT COUNT(*) FROM images").fetchone()[0]

        cur.execute("BEGIN")
        # Nuke every tag row and the join table.
        cur.execute("DELETE FROM image_tags")
        cur.execute("DELETE FROM tags")
        # Reset AI bookkeeping so the next auto-tag pass reclassifies
        # everything from scratch. Guarded: on a pre-v3 DB the
        # columns don't exist yet; the Rust migration will add them
        # (NULL for every row) next time Magpie opens the file,
        # which is functionally identical to running this UPDATE.
        if has_ai_cols:
            cur.execute(
                "UPDATE images SET ai_tagged_at = NULL, ai_tag_hash = NULL"
            )

        # Re-index FTS: mirror what `rebuild_fts_row_tx` in
        # `db/queries.rs` does — for every image, drop the FTS row
        # and re-insert with the current title + filename + empty
        # tags string.
        cur.execute("DELETE FROM images_fts")
        cur.execute(
            "INSERT INTO images_fts(rowid, title, filename, tags) "
            "SELECT id, COALESCE(title, ''), filename, '' FROM images"
        )
        con.commit()

        # Compact the file so the freed pages don't linger.
        con.execute("VACUUM")

        after_tags = cur.execute("SELECT COUNT(*) FROM tags").fetchone()[0]
        after_image_tags = cur.execute("SELECT COUNT(*) FROM image_tags").fetchone()[0]

        print(
            f"OK       {db_path}\n"
            f"           images={image_count}, "
            f"tags: {before_tags} -> {after_tags}, "
            f"image_tags: {before_image_tags} -> {after_image_tags}, "
            f"ai_tagged rows cleared: {before_ai_tagged}\n"
            f"           backup: {backup}"
        )
    except Exception as exc:
        con.rollback()
        print(f"ERROR    {db_path}: {exc}")
        raise
    finally:
        con.close()


def main(argv: list[str]) -> int:
    targets = argv[1:]
    if not targets:
        print("usage: py scripts/wipe_tags.py <path-to-magpie> [...]")
        return 2
    for t in targets:
        wipe(t)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
