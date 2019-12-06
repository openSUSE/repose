#!/usr/bin/python3

from setuptools import find_packages, setup

from repose import __version__

setup(
    name="repose",
    version=__version__,
    packages=find_packages(exclude=["docs", "tests*"]),
    install_requires=["paramiko"],
    tests_require=["pytest"],
    entry_points={"console_scripts": ["repose = repose.main:main"]},
    author="Ondřej Súkup",
    author_email="osukup@suse.cz",
)
