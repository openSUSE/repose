from abc import ABCMeta


class UserMessage(BaseException, metaclass=ABCMeta):
    """
    Message to be displayed to the user
    """

    def __str__(self):
        return self.message

    def __eq__(self, x):
        return str(self) == str(x)

    @classmethod
    def __hash__(cls):
        return hash(cls)


class ErrorMessage(UserMessage, RuntimeError):
    """
    Program error message to be displayed to the user
    """


class UserError(UserMessage, RuntimeError):
    """
    Error, caused by improper usage of the program,
    to be displayed to the user
    """


class ConnectingTargetFailedMessage(UserMessage):
    def __init__(self, hostname, port, reason):
        self.hostname = hostname
        self.reason = reason
        self.port = port

    def __str__(self):
        return "connecting to {}:{} failed: {}".format(
            self.hostname, self.port, self.reason
        )

    def __repr__(self):
        return "<{0} {1!r}:{2!r}>".format(self.__class__, self.hostname, self.reason)


class ConnectingToMessage(UserMessage):
    def __init__(self, hostname):
        self.hostname = hostname

    def __str__(self):
        return "connecting to {0}".format(self.hostname)


class UnsuportedProductMessage(UserMessage):
    def __init__(self, product):
        self.product = product

    def __str__(self):
        message = (
            f"Unknow product {self.product.name} version {self.product.version}.\n"
        )
        message += "Please update products.yaml file with product definition"
        return message
