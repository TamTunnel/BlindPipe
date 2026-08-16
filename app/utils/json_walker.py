from typing import Any, Callable

def walk_and_modify(data: Any, modifier: Callable[[str], str]) -> Any:
    """
    Recursively walks a JSON-compatible data structure and applies `modifier`
    to any string values it finds.
    """
    if isinstance(data, dict):
        return {k: walk_and_modify(v, modifier) for k, v in data.items()}
    elif isinstance(data, list):
        return [walk_and_modify(item, modifier) for item in data]
    elif isinstance(data, str):
        return modifier(data)
    else:
        # Ints, floats, booleans, None, etc. remain unchanged.
        return data
