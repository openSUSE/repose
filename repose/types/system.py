from __future__ import annotations

from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Any

from .refhost.transformations import (
    transform_version_partialy,
)

if TYPE_CHECKING:
    from ..target.parsers import Product


class UnknownSystemError(ValueError):
    pass


@dataclass(frozen=True, slots=True)
class SystemData:
    """Typed internal storage for System.

    ``addons`` is snapshotted into a ``frozenset`` on construction, so
    later mutation of the iterable passed by the caller cannot alter
    this record.

    ``transactional`` marks an immutable / transactional-update host (SL
    Micro, SLE Micro, MicroOS): package operations must go through
    ``transactional-update`` and a reboot, rather than direct ``zypper``.
    """

    base: Product
    addons: frozenset[Product]
    transactional: bool = False

    def __post_init__(self) -> None:
        """Defensively copy ``addons`` so the frozen record owns its state."""
        object.__setattr__(self, "addons", frozenset(self.addons))


@dataclass(eq=False)
class System:
    """Store product information from refhost.

    Used by prettyprint for user and for correct update handling.
    """

    _data: SystemData = field(init=False)

    def __init__(
        self,
        base: Product,
        addons: set[Product] | None = None,
        transactional: bool = False,
    ) -> None:
        """Create a System with a base product and optional addon set.

        Args:
            base: The base product (a ``Product`` dataclass).
            addons: Optional set of addon ``Product`` instances.
            transactional: True for an immutable / transactional-update
                host (package ops go through ``transactional-update`` +
                reboot instead of direct ``zypper``).
        """
        self._data = SystemData(
            base=base,
            addons=frozenset(addons) if addons else frozenset(),
            transactional=transactional,
        )

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

    def is_transactional(self) -> bool:
        """Return True for an immutable / transactional-update host.

        On such hosts (SL Micro, SLE Micro, MicroOS) the root is a
        read-only snapshot, so package install/remove must run through
        ``transactional-update`` and take effect only after a reboot.
        """
        return self._data.transactional

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, System):
            return NotImplemented
        return self._data == other._data

    # __ne__ is automatically derived from __eq__ in Python 3 — no override needed.

    def get_addons(self) -> set[Product]:
        """Return a fresh copy of the addon products.

        Mutating the returned set does not affect this ``System``.
        """
        return set(self._data.addons)

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
