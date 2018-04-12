#!/usr/bin/python3

from setuptools import setup, find_packages
from repose import __version__

setup(
    name="repose",
    version=__version__,
    packages=find_packages(),
    install_requires=['paramiko', 'qamlib'],
    tests_require=['pytest'],

    author="Ondřej Súkup",
    author_email="osukup@suse.cz")
