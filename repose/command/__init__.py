from ._command import Command as Command

# Importing for the side effect of populating ``Command.registry`` via
# ``__init_subclass__``. Order matters: ``remove`` must import before
# ``uninstall`` (Uninstall subclasses Remove), and ``clear`` before
# ``reset`` (Reset subclasses Clear). The alphabetical order below
# satisfies both constraints.
from . import (  # noqa: F401
    add,
    clear,
    install,
    known,
    list,
    remove,
    reset,
    uninstall,
)
