from __future__ import annotations

from typing import TYPE_CHECKING, Any, NamedTuple

from .refhost.transformations import (
    transform_version_partialy,
)

if TYPE_CHECKING:
    from ..target.parsers import Product


class UnknownSystemError(ValueError):
    pass


class SystemData(NamedTuple):
    """Typed internal storage for System — always exactly two fields."""

    base: Product
    addons: set[Product]


class System:
    """Store product information from refhost.

    Used by prettyprint for user and for correct update handling.
    """

    def __init__(self, base: Product, addons: set[Product] | None = None) -> None:
        """Create a System with a base product and optional addon set.

        Args:
            base: The base product (a ``Product`` named tuple).
            addons: Optional set of addon ``Product`` instances.
        """
        self._data = SystemData(base=base, addons=addons if addons else set())

    def __str__(self) -> str:
        suffix = "-modules" if self._data.addons else ""
        base = self._data.base
        return f"{base.name.lower()}{suffix}-{base.version}-{base.arch}"

    def pretty(self) -> list[str]:
        base = self._data.base
        msg = [f"  Base product: {base.name}-{base.version}-{base.arch}"]
        if self._data.addons:
            msg += ["  Installed Extensions and Modules:"]
            msg += [
                f"      Addon: {x.name:<53} - version: {x.version}"
                for x in self._data.addons
            ]
        return msg

    def to_refhost_dict_partially_normalized(self) -> dict[str, Any]:
        return {
            "location": ["some location"],
            "arch": self.arch(),
            "product": self._get_base_dict_partialy_normalized(),
            "addons": self._get_addons_list_partialy_normalized(),
        }

    def to_refhost_dict(self) -> dict[str, Any]:
        return {
            "location": ["some location"],
            "arch": self.arch(),
            "product": self._get_base_dict(),
            "addons": self._get_addons_list(),
        }

    def arch(self) -> str:
        return self._data.base.arch

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, System):
            return NotImplemented
        return self._data == other._data

    # __ne__ is automatically derived from __eq__ in Python 3 — no override needed.

    def get_addons(self) -> set[Product]:
        return self._data.addons

    def get_base(self) -> Product:
        return self._data.base

    def _get_base_dict(self) -> dict[str, str]:
        return {
            "name": self._data.base.name,
            "version": self._data.base.version,
        }

    def _get_addons_list(self) -> list[dict[str, str]]:
        return [{"name": x.name, "version": x.version} for x in self._data.addons]

    def _get_base_dict_partialy_normalized(self) -> dict[str, Any]:
        return {
            "name": self._data.base.name,
            "version": transform_version_partialy(self._data.base.version),
        }

    def _get_addons_list_partialy_normalized(self) -> list[dict[str, Any]]:
        return [
            {"name": x.name, "version": transform_version_partialy(x.version)}
            for x in self._data.addons
        ]

    def flatten(self) -> set[Product]:
        return {self._data.base} | self._data.addons
