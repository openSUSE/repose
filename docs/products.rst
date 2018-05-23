########################################################################
                        Products definiton file 
########################################################################

Products definition file is YAML file usually installed in ``/etc/repose/`` directory
under name **products.yml**.


Repose can load this file from another location with ``-c``, ``--config`` option.


Structure of products file
##########################

standard format rules
---------------------

white space matters

``${version}`` expand to standard version eg. **12-SP1**, **3.1** etc.

``${shortver}`` expands to **12SP3** etc.

``${arch}`` expand to architeccture - **x86_64** etc.


format of products.yml
----------------------

::
  
  name:
    version: &namever
      repo1:
        url: http://something.somevhere/name/${version}/${arch}/
      repo2:
        url: http....
        enabled: true
      default_repos:
        - repo2
     version+1: *namever
     version+2:
       <<: *namever
       repo3:
         url: http


**name** - product name as in product file

**version** - version of product, all known version must be filled

**repo..** - name of repository, usually *pool*, *update*

**url** - url address template

**enabled** - sets autorefresh to true, optional and desired to be true only on update repositories and repositories which are changing (rolling release)

**default_repos** - key in which is list of repositories installed by default to host


