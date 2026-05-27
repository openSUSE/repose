import json

from .utils import blue, green, yellow


class CommandDisplay:
    """Human-readable, color-aware text output for list/known commands."""

    def __init__(self, output):
        self.output = output

    def println(self, msg="", eol="\n"):
        return self.output.write(msg + eol)

    def list_products(self, hostname, port, system):
        self.println(f"{green('Host')}: {yellow(hostname)}:{yellow(port)}")
        for x in system.pretty():
            self.println(x)
        self.println()

    def list_update_repos(self, hostname, port, repos):
        self.println(f"{green('Repositories')} on {blue(hostname)}:{blue(str(port))}")
        for repository in repos:
            self.println(f"{green('REPO name')}: {repository.name}")
            self.println(f"{green('REPO URL')}: {repository.url}")
        self.println()

    def list_known_products(self, products):
        self.println(green("Products known by 'repose':"))
        self.println(" ".join(products))
        self.println()

    @staticmethod
    def __open_yaml():
        from ruamel.yaml import YAML

        yml = YAML(typ="safe", pure=False)
        yml.default_flow_style = False
        yml.explicit_end = True
        yml.explicit_start = True
        yml.indent(mapping=4, sequence=4, offset=2)
        return yml

    def list_products_yaml(self, hostname, system):
        data = system.to_refhost_dict_partially_normalized()
        data["name"] = str(hostname)
        self.__open_yaml().dump(data, self.output)


class JsonCommandDisplay:
    """Newline-delimited JSON output for list/known commands.

    One JSON object per output line. Schema matches the per-event
    envelope used by ``repose.console.Console`` so ``--format=json``
    yields a consistent stream across every subcommand.

    Event shapes:

    - ``{"event": "product", "host", "port", "kind": "base"|"addon",
       "name", "version", "arch"}`` — one per product per host.
    - ``{"event": "repo", "host", "port", "alias", "name", "url",
       "state"}`` — one per repository per host.
    - ``{"event": "known_product", "name"}`` — one per known product.
    - ``{"event": "host_spec", "host", ...}`` — one per host when
      ``--yaml`` is combined with ``--format=json``; carries the same
      payload the YAML dumper would emit.
    """

    def __init__(self, output):
        self.output = output

    def _emit(self, payload: dict) -> None:
        self.output.write(json.dumps(payload) + "\n")

    def list_products(self, hostname, port, system) -> None:
        host = str(hostname)
        base = system.get_base()
        self._emit(
            {
                "event": "product",
                "host": host,
                "port": port,
                "kind": "base",
                "name": base.name,
                "version": base.version,
                "arch": base.arch,
            }
        )
        for addon in system.get_addons():
            self._emit(
                {
                    "event": "product",
                    "host": host,
                    "port": port,
                    "kind": "addon",
                    "name": addon.name,
                    "version": addon.version,
                    "arch": addon.arch,
                }
            )

    def list_update_repos(self, hostname, port, repos) -> None:
        host = str(hostname)
        for repository in repos:
            self._emit(
                {
                    "event": "repo",
                    "host": host,
                    "port": port,
                    "alias": repository.alias,
                    "name": repository.name,
                    "url": repository.url,
                    "state": repository.state,
                }
            )

    def list_known_products(self, products) -> None:
        for name in products:
            self._emit({"event": "known_product", "name": name})

    def list_products_yaml(self, hostname, system) -> None:
        # When --yaml and --format=json are combined, ship the same
        # dict the YAML dumper would emit, JSON-serialised, one
        # document per host.
        data = system.to_refhost_dict_partially_normalized()
        data["name"] = str(hostname)
        self._emit({"event": "host_spec", "host": str(hostname), **data})
