from collections import namedtuple

Product = namedtuple("Product", ("name", "version", "arch"))
Repository = namedtuple("Repository", ("alias", "name", "url", "state"))
