from dataclasses import dataclass


@dataclass(frozen=True, slots=True)
class Product:
    name: str
    version: str
    arch: str


@dataclass(frozen=True, slots=True)
class Repository:
    alias: str
    name: str
    url: str
    state: bool
