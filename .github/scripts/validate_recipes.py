#!/usr/bin/env python3
"""Proverava da su svi recepti u recipes/ ispravan TOML sa obaveznim poljima."""
import sys
import tomllib
from pathlib import Path

ENGINES = {"gptk", "dxmt", "wined3d", "dxvk-moltenvk"}
REQUIRED = {"game": ["id", "name", "store"], "runtime": ["engine"]}
RECOMMENDED = {"game": ["exe"]}


def check(path: Path) -> tuple[list[str], list[str]]:
    try:
        data = tomllib.loads(path.read_text(encoding="utf-8"))
    except tomllib.TOMLDecodeError as e:
        return [f"neispravan TOML: {e}"], []
    errors = []
    for table, keys in REQUIRED.items():
        section = data.get(table)
        if not isinstance(section, dict):
            errors.append(f"nedostaje tabela [{table}]")
            continue
        errors += [f"nedostaje {table}.{k}" for k in keys if k not in section]
    game_id = data.get("game", {}).get("id")
    if game_id is not None and game_id != path.stem:
        errors.append(f"game.id '{game_id}' se ne poklapa sa imenom fajla '{path.stem}'")
    engine = data.get("runtime", {}).get("engine")
    if engine is not None and engine not in ENGINES:
        errors.append(f"runtime.engine '{engine}' nije jedan od {sorted(ENGINES)}")
    if data.get("game", {}).get("store") == "steam" and "steam_app_id" not in data["game"]:
        errors.append("store = 'steam' trazi game.steam_app_id")
    warnings = [
        f"preporuceno polje {table}.{k} nije popunjeno"
        for table, keys in RECOMMENDED.items()
        for k in keys
        if k not in data.get(table, {})
    ]
    return errors, warnings


def main() -> int:
    root = Path(sys.argv[1] if len(sys.argv) > 1 else "recipes")
    recipes = sorted(root.glob("*.toml"))
    if not recipes:
        print(f"nema recepata u {root}/", file=sys.stderr)
        return 1
    failed = False
    for path in recipes:
        errors, warnings = check(path)
        for warn in warnings:
            print(f"::warning file={path}::{warn}")
        if errors:
            failed = True
            for err in errors:
                print(f"::error file={path}::{err}")
        else:
            print(f"ok  {path}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
