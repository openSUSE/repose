import importlib
from pathlib import Path
from ._command import Command as Command

_rootdir = Path(__file__).resolve().parent
cmd_list = []

for pth in _rootdir.iterdir():
    # list all ".py"
    if pth.is_file() and pth.suffix == ".py":
        modname = pth.stem
    else:
        continue
    # skip things like __init__, __pycache__, __main__ , _commad ...
    if modname.startswith("_"):
        continue
    try:
        module = importlib.import_module("." + modname, "repose.command")
    except BaseException:
        continue
    # register classes
    __klzs = [x for x in dir(module) if hasattr(getattr(module, x), "command")]
    cmd_list += __klzs
    for x in __klzs:
        globals()[x] = getattr(module, x)
