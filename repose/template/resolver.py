import logging
from copy import deepcopy
from difflib import get_close_matches
from string import Template
from typing import NamedTuple

from ..messages import UnsuportedProductMessage
from ..target.parsers import Product
from ..types.repa import Repa
from ..types.system import System


class Repos(NamedTuple):
    name: str
    url: str
    refresh: bool


logger = logging.getLogger("repose.template.resolver")


class Repoq:
    """resolve and return template data for requested repositories"""

    def __init__(self, template: dict) -> None:
        self.template = template

    def solve_repa(self, orepa: Repa, base: Product) -> dict[str, list[Repos]]:
        """.. returns needed repositories for REPA
        ::param:: orepa - instance of Repa object
        ::param:: base -- System.get_base()
        ::return:: {release-file:[(name, url, refresh-state,),]}
        ::raises:: ValueError when the REPA cannot be resolved (unknown
        product or version, missing template key such as
        ``default_repos``, or an unknown ``$placeholder`` in a repo URL)
        """
        repa = deepcopy(orepa)
        if repa.product not in self.template:
            candidates = get_close_matches(
                repa.product or "", list(self.template.keys())
            )
            error_msg = f"Not known product: {repa.product}"
            if candidates:
                error_msg += f" Did you mean {candidates[0]}?"
            raise ValueError(error_msg)
        if repa.arch is None:
            repa.arch = base.arch
        if repa.version is None:
            repa.version = base.version

        template = self.template.copy()
        result = {}

        if repa.version in template[repa.product]:
            subtemplate = template[repa.product][repa.version]
            version = repa.version
        elif repa.baseversion in template[repa.product]:
            subtemplate = template[repa.product][repa.baseversion]
            version = repa.baseversion
        else:
            raise ValueError(
                f"Unknow version: {repa.version} for product: {repa.product}"
            )
        assert version is not None
        name = f"{repa.product}:{version}::"

        # used by for example QA:SLE projects
        shortversion = version.replace("-", "")

        try:
            if repa.repo:
                logger.debug("Return data for %s - %s", name, repa.repo)
                url = Template(
                    subtemplate.get(repa.repo, {"url": "http://empty.url"})["url"]
                ).substitute(version=version, arch=repa.arch, shortver=shortversion)
                rname = name + repa.repo
                refresh = subtemplate.get(repa.repo, {}).get("enabled", False)
                result[repa.product] = [Repos(rname, url, refresh)]
            else:
                rlist = []
                for x in subtemplate["default_repos"]:
                    logger.debug("Return data for %s - %s", name, x)
                    url = Template(
                        subtemplate.get(x, {"url": "http://empty.url"})["url"]
                    ).substitute(version=version, arch=repa.arch, shortver=shortversion)
                    rname = name + x
                    refresh = subtemplate.get(x, {}).get("enabled", False)
                    rlist.append(Repos(rname, url, refresh))
                result[repa.product] = rlist
        except KeyError as error:
            raise ValueError(
                f"Cannot resolve REPA {name}{repa.repo or ''}: "
                f"missing template key or URL placeholder {error}"
            ) from error

        return result

    def solve_product(self, products: System) -> dict[str, list[Repos]]:
        """.. returns needed repositories for products from system :D
        ::param:: products -- instance of System object
        ::return:: { release-file:[(name, url, refresh-state,),] ..}"""
        installed = products.flatten()
        template = self.template.copy()
        result = {}
        for product in installed:
            name = f"{product.name}:{product.version}::"
            rlist = []
            try:
                for repo in template[product.name][product.version]["default_repos"]:
                    url = Template(
                        template[product.name][product.version].get(
                            repo, {"url": "http://empty.url"}
                        )["url"]
                    ).substitute(
                        version=product.version,
                        arch=product.arch,
                        shortver=product.version.replace("-", ""),
                    )
                    rlist.append(
                        Repos(
                            name + repo,
                            url,
                            template[product.name][product.version]
                            .get(repo, {})
                            .get("enabled", False),
                        )
                    )
                result.update({product.name: rlist})
            except KeyError:
                raise UnsuportedProductMessage(product)
        return result
