from dataclasses import dataclass, field


@dataclass
class Repa:
    """Object holding REPA data.

    Supports two construction shapes for backward compatibility:

    * ``Repa("SLES:15-SP3:x86_64:update")`` — colon-separated string
      (legacy form used by the CLI parser and existing tests).
    * ``Repa(product="SLES", version="15-SP3", arch="x86_64",
      repo="update")`` — explicit dataclass kwargs.

    Derived fields ``baseversion`` and ``smallver`` are populated by
    ``__post_init__`` from ``version``.
    """

    product: str | None = None
    version: str | None = None
    arch: str | None = None
    repo: str | None = None
    baseversion: str | None = field(init=False, default=None)
    smallver: str | None = field(init=False, default=None)

    def __init__(
        self,
        repa: str | None = None,
        *,
        product: str | None = None,
        version: str | None = None,
        arch: str | None = None,
        repo: str | None = None,
    ) -> None:
        if isinstance(repa, str):
            parts: list[str | None] = list(repa.split(":"))
            if len(parts) > 4:
                raise ValueError("REPA can't have more than 4 components")
            parts += [None] * (4 - len(parts))
            product = parts[0] or None
            version = parts[1] or None
            arch = parts[2] or None
            repo = parts[3] or None
        self.product = product
        self.version = version
        self.arch = arch
        self.repo = repo
        self.__post_init__()

    def __post_init__(self) -> None:
        if self.version and "-SP" in self.version:
            self.smallver = "-{}".format(self.version.split("-")[-1])
            self.baseversion = self.version.split("-")[0]
        elif self.version:
            self.smallver = None
            self.baseversion = self.version
        else:
            self.smallver = None
            self.baseversion = None
