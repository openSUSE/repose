from typing import NamedTuple


class Product(NamedTuple):
    name: str
    version: str
    arch: str


class Repository(NamedTuple):
    alias: str
    name: str
    url: str
    state: bool
