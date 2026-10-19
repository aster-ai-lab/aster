"""Background-only CUA guard.

Starting refuses when A3S_GUI_ALLOW_MOUSE is set. A click, key, text insert,
or hotkey refuses unless its delivery mode is background. Pointer tools are
refused. This module does not open ToWork and does not move the pointer.
"""

from __future__ import annotations

import os
from typing import Any, Callable


MOUSE_ENV = "A3S_GUI_ALLOW_MOUSE"
POINTER_TOOLS = frozenset({"move_mouse", "drag", "scroll", "raise_window", "set_frontmost"})
ACTION_TOOLS = frozenset({"click", "press_key", "type_text", "hotkey"})


class GuardError(RuntimeError):
    pass


def mouse_value(environ: dict[str, str] | None = None) -> str | None:
    env = os.environ if environ is None else environ
    value = env.get(MOUSE_ENV)
    if value is None or value == "":
        return None
    return value


def refuse_mouse(environ: dict[str, str] | None = None) -> None:
    value = mouse_value(environ)
    if value is not None:
        raise GuardError(f"A3S_GUI_ALLOW_MOUSE={value}")


def refuse_action(name: str, arguments: dict[str, Any]) -> None:
    if name in POINTER_TOOLS:
        raise GuardError(f"refusing pointer tool {name}")
    mode = arguments.get("delivery_mode")
    if name in ACTION_TOOLS and mode != "background":
        raise GuardError(f"refusing {name} without background delivery")
    if mode not in (None, "background"):
        raise GuardError(f"refusing delivery {mode}")


def reported_delivery(result: dict[str, Any], arguments: dict[str, Any]) -> str:
    structured = result.get("structuredContent") or {}
    delivery = structured.get("delivery") or {}
    mode = delivery.get("mode") if isinstance(delivery, dict) else None
    if mode:
        return str(mode)
    fallback = arguments.get("delivery_mode")
    return str(fallback) if fallback else ""


class GuardState:
    """Check every tool call. `invoke` runs only after the check passes."""

    def __init__(self, environ: dict[str, str] | None = None) -> None:
        refuse_mouse(environ)
        self.environ = environ
        self.non_background = 0
        self.actions: list[str] = []

    def call(self, invoke: Callable[[], dict[str, Any]], name: str, arguments: dict[str, Any]) -> dict[str, Any]:
        refuse_mouse(self.environ)
        refuse_action(name, arguments)
        result = invoke()
        if name in ACTION_TOOLS:
            delivery = reported_delivery(result, arguments)
            self.actions.append(f"ACTION {name} delivery_mode: {delivery}")
            if delivery != "background":
                self.non_background += 1
                raise GuardError(f"driver reported delivery {delivery}")
        return result
