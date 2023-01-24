class Repa(object):
    """Object holding REPA data"""

    def __init__(self, repa):
        self.__version = None
        self.__parse(repa)

    def __parse(self, repa):
        data = repa.split(":")
        lenght = len(data)
        if lenght > 4:
            raise ValueError("REPA can't have more than 4 components")
        elif lenght < 4:
            for x in range(4 - lenght):
                data.append(None)
        self.product = data[0] if data[0] else None
        self.version = data[1] if data[1] else None
        self.arch = data[2] if data[2] else None
        self.repo = data[3] if data[3] else None

    @property
    def version(self):
        return self.__version

    @version.setter
    def version(self, value):
        self.__version = value
        if self.__version and "-SP" in self.__version:
            self.smallver = "-{}".format(self.__version.split("-")[-1])
            self.baseversion = self.__version.split("-")[0]
        elif self.__version and not "-SP" in self.__version:
            self.smallver = None
            self.baseversion = self.__version
        else:
            self.smallver = None
            self.baseversion = None

    def __repr__(self):
        return "<object REPA: {}_{}_{}_{}>".format(
            self.product, self.version, self.arch, self.repo
        )
