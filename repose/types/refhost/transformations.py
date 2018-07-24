
__transform_table = {"SLES": "sles",
                     "SUSE-SLES": "sles",
                     "sle-module-basesystem": "Basesystem",
                     "sle-module-development-tools": "Development-Tools",
                     "sle-module-desktop-applications": "Desktop-Applications",
                     "sle-module-containers": "Containers",
                     "ses": "sles",
                     "SUSE-Manager-Server": "manager",
                     "SUSE-Manager-Proxy": "manager-proxy",
                     "CAASP": "caasp",
                     "sle-11-WebYaST": "webyast",
                     "SUSE_SLES": "sles",
                     "sle-studioonsiterunner": "studiorunner",
                     "sle-we": "we",
                     "sle-studioonsite": "studio",
                     "sle-smt": "smt",
                     "sle-slms": "slms",
                     "sle-sdk": "sdk",
                     "sle-live-patching": "Live-Patching",
                     "SLE-Manager-Tools": "manager-client",
                     "SUSE_SLES_SAP": "sap-aio",
                     "SLES_SAP": "sap-aio",
                     "hpe-helion-openstack": "cloud",
                     "suse-openstack-cloud": "cloud",
                     "openstack-cloud-magnum-orchestration": " cloud",
                     "suse-openstack-cloud-crowbar": "cloud",
                     "sle-bsk": "bsk",
                     "SLED": "sled",
                     "sle-ha": "hae",
                     "sle-ha-geo": "hae",
                     "sle-hae": "hae",
                     "sle-hae-geo": "hae",
                     "SLES_HPC": "HPC",
                     "sle-module-hpc": "HPC",
                     "sle-module-adv-systems-management": "Adv-Systems-Management",
                     "sle-module-certifications": "Certifications",
                     "sle-module-certifications-2017": "Certifications-2017",
                     "sle-module-ha": "HA",
                     "sle-module-live-patching": "Live-Patching",
                     "sle-module-legacy": "Legacy",
                     "SLES-LTSS": "sles",
                     "openSUSE": "opensuse",
                     "security": "security",
                     "teradata": "teradata",
                     "sle-module-pubcloud": "pubcloud",
                     "sle-module-public-cloud": "Public-Cloud",
                     "sle-module-sap-applications": "SAP-Applications",
                     "sle-module-server-applications": "Server-Applications",
                     "sle-module-toolchain": "Toolchain",
                     "sle-module-web-scripting": "Web-Scripting",
                     "PackageHub": "PackageHub",
                     "sle-module-cap-tools": "Cap-Tools",
                     "SUSE-Linux-Enterprise-RT": "rt",
                     "qa": "qa"}


def transform_version(version, product):
    # transform versions
    minor = None
    if "-SP" in version:
        major, minor = version.split("-SP")
        minor = "sp"+minor
        major = int(major)
    elif product == 'ses':
        if version == '4':
            major = 12
            minor = "sp2"
        elif version == '5':
            major = 12
            minor = 'sp3'
        elif version == '6':
            major = 15
    elif "." in version and product != 'ses':
        major, minor = version.split(".")
        major = int(major)
        minor = int(minor)
    elif version == "ALL":
        major = "ALL"
    else:
        major = int(version)

    ret = {'major': major}

    if minor is not None:
        ret.update({'minor': minor})

    return ret


def transform_product(product):
    return __transform_table.get(product, "Unknown Product")
