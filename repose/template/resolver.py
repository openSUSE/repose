
import logging
from string import Template
from collections import namedtuple
from copy import deepcopy

from ..messages import UnsuportedProductMessage

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
            url = Template(subtemplate.get(repa.repo, {'url': "http://empty.url"})['url']).substitute(
                version=version, arch=repa.arch, shortver=shortversion)
            rname = name + repa.repo
            refresh = subtemplate.get(repa.repo, {}).get('enabled', False)
            result[repa.product] = [Repos(rname, url, refresh)]
        else:
            rlist = []
            for x in subtemplate['default_repos']:
                logger.debug("Return data for {} - {}".format(name, x))
                url = Template(subtemplate.get(x, {'url': "http://empty.url"})
                               ['url']).substitute(version=version, arch=repa.arch, shortver=shortversion)
                rname = name + x
                refresh = subtemplate.get(x, {}).get('enabled', False)
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
            try:
                for repo in template[product.name][product.version]['default_repos']:
                    url = Template(
                        template[product.name][product.version].get(repo, {"url": "http://empty.url"})['url']).substitute(
                        version=product.version, arch=product.arch, shortver=product.version.replace('-',''))
                    rlist.append(Repos(name + repo, url, template[product.name]
                                       [product.version].get(repo, {}).get('enabled', False)))
                result.update({product.name: rlist})
            except KeyError:
                raise UnsuportedProductMessage(product)
        return result
