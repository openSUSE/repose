def transform_version_partialy(version: str) -> dict[str, int | str] | str:
    """Normalise a version string into a dict with 'major' and optional 'minor' keys.

    Args:
        version: Version string in one of the forms ``"15-SP3"``, ``"15.3"``,
            ``"ALL"``, or a plain integer string such as ``"15"``.

    Returns:
        Dictionary with at least a ``"major"`` key, and a ``"minor"`` key when
        the version contains a service-pack or minor component. Versions that
        do not fit those SUSE shapes — an empty string, a non-numeric value
        such as ``"tumbleweed"``, or a multi-part one such as Alpine's
        ``"3.19.1"`` (all legitimate os-release ``VERSION_ID`` shapes) — are
        returned unchanged instead of raising.
    """
    minor: int | str | None = None
    major: int | str
    try:
        if "-" in version:
            major_str, minor_str = version.split("-", 1)
            major = int(major_str)
            minor = minor_str
        elif "." in version:
            major_str, minor_str = version.split(".", 1)
            major = int(major_str)
            minor = int(minor_str)
        elif version == "ALL":
            major = "ALL"
        else:
            major = int(version)
    except ValueError:
        # Best effort: pass unnormalisable versions through unchanged
        # rather than aborting whole-report rendering.
        return version

    ret: dict[str, int | str] = {"major": major}

    if minor is not None:
        ret["minor"] = minor

    return ret
