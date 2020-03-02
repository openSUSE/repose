
from .refhost.transformations import transform_version, transform_product


class UnknownSystemError(ValueError):
    pass


class System:
    """
    Store product information from refhost
    used by prettyprint for user and
    for correct update handling
    """

    def __init__(self, base, addons=None):
        """
        base: type Product(name, version, arch)
        addons: type set of Product(name, version, arch)
        """
        # TODO: check for correctness of base and addons types
        addons = addons if addons else set()
        self._data = {"base": base, 'addons': addons}

    def __str__(self):
        addons = "-modules" if self._data['addons'] else ''
        msg = self._data['base'].name.lower()
        msg += addons
        msg += "-" + self._data['base'].version
        msg += "-" + self._data['base'].arch
        return msg

    def pretty(self):
        msg = ["  Base product: {}-{}-{}".format(self._data['base'].name,
                                                 self._data['base'].version, self._data['base'].arch)]
        if self._data['addons']:
            msg += ['  Installed Extensions and Modules:']
            msg += ['      Addon: {:<53} - version: {}'.format(x.name, x.version) for x in self._data['addons']]
        return msg

    def to_refhost_dict_normalized(self):
        ret = {}
        # simple values
        ret["location"] = ["some location"]
        ret["arch"] = self.arch()
        ret['product'] = self._get_base_dict_normalized()
        ret['addons'] = self._get_addons_list_normalized()
        return ret

    def to_refhost_dict(self):
        ret = {}
        # simple values
        ret["location"] = ["some location"]
        ret["arch"] = self.arch()
        ret['product'] = self._get_base_dict()
        ret['addons'] = self._get_addons_list()
        return ret

    def arch(self):
        return self._data['base'].arch

    def __eq__(self, other):
        return self._data == other._data

    def __ne__(self, other):
        return not self.__eq__(other)

    def get_addons(self):
        return(self._data['addons'])

    def get_base(self):
        return(self._data['base'])

    def _get_base_dict(self):
        ret = {"name": self._data['base'].name}
        ret.update({"version": self._data['base'].version})
        return ret

    def _get_addons_list(self):
        return [{"name": x.name, "version": x.version}
                for x in self._data['addons']]

    def _get_base_dict_normalized(self):
        ret = {"name": transform_product(self._data['base'].name)}
        ret.update({"version": transform_version(self._data['base'].version, self._data['base'].name)})
        return ret

    def _get_addons_list_normalized(self):
        return [{"name": transform_product(x.name), "version": transform_version(x.version, x.name)}
                for x in self._data['addons']]

    def flatten(self):
        flat = {self._data['base']}
        flat.update(self._data['addons'])
        return flat
