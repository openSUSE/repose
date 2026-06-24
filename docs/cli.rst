######################
Command Line Interface
######################

.. contents::
  :depth: 4

Introduction
============

The REPOSE command line is realized with the `Typer <https://typer.tiangolo.com>`_
framework, close but not same as old ``repose-0.2*.*``.

Command line has few global options and bunch of commands with own options.

For global options and also for commands is available ``-h`` / ``--help``
option, which opens the corresponding man page.

Shell completion for bash, zsh, and fish ships built in. Install it once
into your shell's rc file with ``repose --install-completion bash`` (or
``zsh`` / ``fish``); ``repose --show-completion zsh`` prints the script
without installing it. Subcommands, flags, and ``REPA`` product prefixes
read from ``products.yml`` are completed.


Common Argument Types
=====================


.. option:: -t HOST, --target HOST


  Address of the target host (should be the FQDN).
  
  In most cases ``-t`` is an required argument; can be used multiple times.

  Can be ``user@fqdn:port``, ``fqdn:port``, ``user@fqdn``, ``fqdn`` or simply 
  ip addres, hostname.

  By default it uses ``root`` as user and ``22`` as port. User must have **root**
  privilegies on refhost.

.. option:: REPA

**Repository patterns** accepted by **repose** :

 |
 | [**P**][:[**V**][:[**A**][:[**T**]]]]

 Empty segments are treated as wildcards or default to information gleaned from */etc/products.d/baseproduct*, depending on context. Some useful variants:

 |
 | .  **P**
 | ·  **P**:**V**:**A**:**T**
 | ·  **P**:**V**:**A**
 | ·  **P**:**V**
 | ·  **P**:**V**::**T**
 | ·  **P**:::**T**
 | ·  :::**T**

**P** is product name as is in product file

**V** version

**A** architecture

**T** repo type as defined in ``products.yml``, for example ``pool``, ``update`` or ``ltss``

Global Options
==============


All global options are optional.

.. option:: -h, --help

  Display the help / man page.

.. option:: -n,  --print

  Print commands for HOST instead running on HOST

.. option:: -d, --debug
  
  Debug mode, is mutualy exclusive with ``-q``

.. option:: -q, --quiet
  
  Suppres messages from repose, only errors / warnings will show.
  Mutualy exclusive with ``-d``

.. option:: -V, --version

  Prints version information and exits.

.. option:: -c CONFIG,  --config CONFIG

  Path for config yaml file. Is optional and by default points to **/etc/repose/products.yml**

.. option:: --no-color

  Disable ANSI color in console output. The ``NO_COLOR`` environment
  variable is also honored, and the legacy ``COLOR=always|never``
  variable overrides terminal detection. By default color is enabled
  only when stdout is a terminal.

.. option:: --format {text,json}

  Console output format: ``text`` (default, human-readable) or ``json``
  (newline-delimited JSON, one event per line, suitable for scripts).

.. option:: --strict-host-key-checking {yes,accept-new,no,off}

  SSH host-key policy following OpenSSH semantics (default:
  ``accept-new``). ``yes`` refuses unknown hosts; ``accept-new`` accepts
  unknown hosts on first contact and records them, but rejects changed
  keys; ``no`` / ``off`` accepts both unknown and changed keys.

.. option:: --known-hosts PATH

  Path to a custom ``known_hosts`` file (overrides
  ``~/.ssh/known_hosts``).

.. option:: --ssh-backend {asyncssh,paramiko}

  SSH backend implementation: ``asyncssh`` (default, structured
  concurrency, no thread pool) or ``paramiko`` (legacy, available for
  one release as a safety net while ``asyncssh`` settles).


Commands
========

known-products
--------------

::
  
  known-products

List all known products by repose, defined in products.yml


add
---

::
  
  add [-h] -t HOST REPA [REPA ...] [--probe-timeout SECONDS] [--no-probe]

Add repository to HOST specified by REPA

Before applying changes the candidate repository URLs are probed in
parallel; ``--probe-timeout`` (default ``5`` seconds) bounds each probe
and ``--no-probe`` skips probing entirely.


remove
------

::
  
  remove [-h] -t HOST REPA [REPA ...]

Remove repositories from HOST specified by REPA


reset
-----

::
  
  reset [-h] -t HOST [--probe-timeout SECONDS] [--no-probe]

Reset HOST repositories to default state based on products installed on host.
It has always two phases - clear all repositories from HOST and readd all valid
repositories according to installed products

The re-added repositories are probed for liveness; ``--probe-timeout``
(default ``5`` seconds) bounds each probe and ``--no-probe`` skips
probing entirely.


clear
-----

::
  
  clear [-h] -t HOST

Clear all repositories from HOST


install
-------

::

  install [-h] -t HOST REPA [REPA ...] [--probe-timeout SECONDS] [--no-probe] [--no-reboot]

Add repositories and install products from  HOST corresponding to REPA

Candidate repository URLs are probed before install; ``--probe-timeout``
(default ``5`` seconds) bounds each probe and ``--no-probe`` skips
probing entirely.

On transactional hosts (SL Micro / SLE Micro / MicroOS) the products are
installed via ``transactional-update`` and the host is rebooted,
reconnected and verified by default. ``--no-reboot`` stages the change
without rebooting (no-op on non-transactional hosts).


uninstall
---------

::

  uninstall [-h] -t HOST REPA [REPA ...] [--no-reboot]

Remove repositories and uninstall product from HOST corresponding to REPA

On transactional hosts the products are removed via
``transactional-update`` and the host is rebooted, reconnected and
verified by default; ``--no-reboot`` stages the change.


Transactional systems
---------------------

Repose auto-detects immutable / transactional hosts (presence of a
``transactional-update.conf``). Repository operations stay plain
``zypper`` (the repo config is on a writable overlay); only product
``install``/``uninstall`` is routed through ``transactional-update`` and
followed by a reboot + reconnect + verification, unless ``--no-reboot``
is given.


list-products
-------------

::
  
  list-products [-h] [--yaml] -t HOST

Show installed products on HOST

.. option:: --yaml
  
  Print output in yaml format and transformed for refhosts yaml preparation


list-repos
----------

::
  
  list-repos [-h] -t HOST

Show repositories on HOST
