
import logging
from string import Template
from collections import namedtuple
from copy import deepcopy

Repos = namedtuple("Repos", ("name", "url", "refresh"))

logger = logging.getLogger('repose.template.resolver')


class Repoq(object):
    """ resolve and return template data for requested repositories """

    def __init__(self, template):
        self.template = template

    def solve_repa(self, orepa, base):
        """ .. returns needed repositories for REPA
        ::param:: orepa - instance of Repa object
        ::param:: base -- System.get_base()
        ::return:: {release-file:[(name, url, refresh-state,),]} """
        repa = deepcopy(orepa)
        if repa.product not in self.template:
            raise ValueError("Not known product: {}".format(repa.product))
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
            raise ValueError("Unknow version: {} for product: {}".format(repa.version, repa.product))
        name = "{}:{}::".format(repa.product, version)

        # used by for example QA:SLE projects
        shortversion = version.replace("-", "")

        if repa.repo:
            logger.debug("Return data for {} - {}".format(name, repa.repo))
            url = Template(subtemplate[repa.repo][0]).substitute(version=version, arch=repa.arch, shortver=shortversion)
            rname = name + repa.repo
            refresh = subtemplate[repa.repo][1]
            result[repa.product] = [Repos(rname, url, refresh)]
        else:
            rlist = []
            for x in subtemplate['default_repos']:
                logger.debug("Return data for {} - {}".format(name, x))
                url = Template(subtemplate[x][0]).substitute(version=version, arch=repa.arch, shortver=shortversion)
                rname = name + x
                refresh = subtemplate[x][1]
                rlist.append(Repos(rname, url, refresh))
            result[repa.product] = rlist

        return result

    def solve_product(self, products):
        """ .. returns needed repositories for products from system :D
        ::param:: products -- instance of System object
        ::return:: { release-file:[(name, url, refresh-state,),] ..} """
        installed = products.flatten()
        template = self.template.copy()
        result = {}
        for product in installed:
            name = "{}:{}::".format(product.name, product.version)
            rlist = []
            for repo in template[product.name][product.version]['default_repos']:
                url = Template(
                    template[product.name][product.version][repo][0]).substitute(
                    version=product.version, arch=product.arch)
                rlist.append(Repos(name + repo, url, template[product.name][product.version][repo][1]))
            result.update({product.name: rlist})

        return result