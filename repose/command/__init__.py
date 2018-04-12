
import importlib
import os
import os.path
from ._command import Command

_rootdir = os.path.dirname(os.path.realpath(__file__))
cmd_list = []

for name in os.listdir(_rootdir):
    # list all ".py"
    path = os.path.join(_rootdir, name)
    if os.path.isfile(path) and name.endswith(".py"):
        modname = name[:-3]
    else:
        continue
    # skip things like __init__, __pycache__, __main__ , _commad ...
    if modname.startswith("_"):
        continue
    try:
        module = importlib.import_module("." + modname, 'repose.command')
    except BaseException:
        continue
    # register classes
    __klzs = [x for x in dir(module) if hasattr(getattr(module, x), "command")]
    cmd_list += __klzs
    for x in __klzs:
        globals()[x] = getattr(module, x)
