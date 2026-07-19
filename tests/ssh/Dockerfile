FROM alpine:3.22.1@sha256:4bcff63911fcb4448bd4fdacec207030997caf25e9bea4045fa6c8c44de311d1

RUN apk add --no-cache openssh \
    && adduser -D -u 1000 repose \
    && passwd -d repose \
    && mkdir -p /etc/products.d /run/sshd \
    && chown repose:repose /home/repose \
    && printf '%s\n' \
        'Port 22' \
        'ListenAddress 0.0.0.0' \
        'PasswordAuthentication no' \
        'KbdInteractiveAuthentication no' \
        'PermitRootLogin no' \
        'PubkeyAuthentication yes' \
        'AuthorizedKeysFile /run/sshd/authorized_keys' \
        'Subsystem sftp internal-sftp' \
        'AllowUsers repose' \
        > /etc/ssh/sshd_config

COPY tests/vectors/refhosts/sles-16-0/products.d/ /etc/products.d/
COPY tests/vectors/refhosts/sles-16-0/zypper-x-lr.xml /usr/local/share/zypper-x-lr.xml

RUN ln -s SLES.prod /etc/products.d/baseproduct \
    && printf '%s\n' \
        '#!/bin/sh' \
        'if [ "$*" = "-x lr" ]; then' \
        '  cat /usr/local/share/zypper-x-lr.xml' \
        '  exit 0' \
        'fi' \
        'printf "unsupported zypper arguments: %s\\n" "$*" >&2' \
        'exit 2' \
        > /usr/local/bin/zypper \
    && chmod 0755 /usr/local/bin/zypper

EXPOSE 22
HEALTHCHECK --interval=1s --timeout=3s --start-period=1s --retries=30 \
    CMD ssh-keyscan -T 2 -p 22 127.0.0.1 >/dev/null 2>&1 || exit 1

CMD ["sh", "-c", "cp /fixture/authorized_keys /run/sshd/authorized_keys && cp /fixture/host_key /run/sshd/host_key && chown repose:repose /run/sshd/authorized_keys && chmod 0600 /run/sshd/authorized_keys /run/sshd/host_key && exec /usr/sbin/sshd -D -e -f /etc/ssh/sshd_config -h /run/sshd/host_key"]
