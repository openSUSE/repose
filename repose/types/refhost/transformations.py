def transform_version_partialy(version):
    minor = None
    if "-" in version:
        major, minor = version.split("-")
        major = int(major)
    elif "." in version:
        major, minor = version.split(".")
        major = int(major)
        minor = int(minor)
    elif version == "ALL":
        major = "ALL"
    else:
        major = int(version)

    ret = {"major": major}

    if minor is not None:
        ret.update({"minor": minor})

    return ret
