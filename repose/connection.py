
import errno
import os
import select
import socket
import getpass

from traceback import format_exc
import logging

import paramiko

logger = logging.getLogger("repose.connection")


class CommandTimeout(Exception):
    """remote command timeout exception
    returns timed out remote command as __str__
    """

    def __init__(self, command=None):
        self.command = command

    def __str__(self):
        return repr(self.command)


class Connection(object):

    """Manage ssh/sftp connection"""

    def __init__(self, hostname, username, port, timeout=120):
        """ openSSH channel to the specified host

        Tries AuthKey Authentication and falls back to password mode
        in case of errors.
        If a connection can't be established (host not available, wrong password/key)
        exceptions are reraised from the ssh subsystem and need to be catched
        by the caller.
        """

        self.username = username
        self.hostname = hostname
        try:
            self.port = int(port)
        except Exception:
            self.port = 22

        self.timeout = timeout

        self.client = paramiko.SSHClient()

    def __repr__(self):
        return "<{} object username={} hostname={} port={}>".format(
            self.__class__.__name__,
            self.username,
            self.hostname,
            self.port)

    def __load_keys(self):
        self.client.load_system_host_keys()
        # Dont check host keys --> StrictHostChecking no
        self.client.set_missing_host_key_policy(
            paramiko.AutoAddPolicy())

    def connect(self):
        cfg = paramiko.config.SSHConfig()
        self.__load_keys()

        try:
            with open(os.path.expanduser("~/.ssh/config")) as fd:
                cfg.parse(fd)
        except IOError as e:
            if e.errno != errno.ENOENT:
                logger.warning(e)
        opts = cfg.lookup(self.hostname)

        try:
            logger.debug('connecting to {}:{}'.format(self.hostname, self.port))
            # if this fails, the user most likely has none or an outdated
            # hostkey for the specified host. checking back with a manual
            # "ssh root@..." invocation helps in most cases.
            self.client.connect(
                hostname=opts.get(
                    'hostname', self.hostname) if 'proxycommand' not in opts else self.hostname, port=int(
                    opts.get(
                        'port', self.port)), username=opts.get(
                    'user', self.username), key_filename=opts.get(
                        'identityfile', None), sock=paramiko.ProxyCommand(
                             opts['proxycommand']) if 'proxycommand' in opts else None, )

        except (paramiko.AuthenticationException, paramiko.BadHostKeyException):
            # if public key auth fails, fallback to a password prompt.
            # other than ssh, mtui asks only once for a password. this could
            # be changed if there is demand for it.
            logger.warning(
                'Authentication failed on {}: AuthKey missing.'.format(
                    self.hostname))
            logger.warning('Trying manually, please enter the root password')
            password = getpass.getpass()

            try:
                # try again with password auth instead of public/private key
                self.client.connect(
                    hostname=opts.get('hostname', self.hostname) if 'proxycommand' not in opts else self.hostname,
                    port=int(opts.get('port', self.port)),
                    username=opts.get('user', self.username),
                    password=password,
                    sock=paramiko.ProxyCommand(
                        opts['proxycommand']) if 'proxycommand' in opts else None, )

            except paramiko.AuthenticationException:
                # if a wrong password was set, don't connect to the host and
                # reraise the exception hoping it's catched somewhere in an
                # upper layer.
                logger.error(
                    'Authentication failed on {}: wrong password'.format(
                        self.hostname))
                raise
        except paramiko.SSHException:
            # unspecified general SSHException. the host/sshd is probably not
            # available.
            logger.error(
                'SSHException while connecting to {}'.format(self.hostname))
            raise
        except Exception as error:
            # general Exception
            logger.error('{}: {}'.format(self.hostname, error))
            raise

    def reconnect(self):
        if not self.is_active():
            logger.debug('lost connection to {}:{}, reconnecting'.format(
                self.hostname, self.port)
            )

            self.connect()

            assert(self.is_active())

    def new_session(self):
        logger.debug(
            'Creating new session at {}:{}'.format(self.hostname, self.port))
        try:
            transport = self.client.get_transport()
            transport.set_keepalive(60)

            session = transport.open_session()
            session.setblocking(0)
            session.settimeout(0)

        except paramiko.SSHException:
            logger.debug(
                'Creating of new session at {}:{} failed'.format(
                    self.hostname, self.port))
            if 'session' in locals():
                session.close()
            session = None
        return session

    @staticmethod
    def close_session(session=None):
        """close the current session"""
        if session:
            try:
                session.shutdown(2)
                session.close()
            except BaseException:
                # pass all exceptions since the session is already closed or broken
                pass

    def __run_command(self, command):
        """ open new session and run command in it

        parameter: command -> str
        result: Succes - session instance with running command
                Fail - False
        """

        try:
            session = self.new_session()
            session.exec_command(command)
        except (AttributeError, paramiko.ChannelException, paramiko.SSHException):
            if 'session' in locals():
                if isinstance(session, paramiko.channel.Channel):
                    self.close_session(session)
            return False
        return session

    def run(self, command, lock=None):
        """run command over SSH channel

        Blocks until command terminates. returncode of issued command is returned.
        In case of errors, -1 is returned.

        If the connection hits the timeout limit, the user is asked to wait or
        cancel the current command.

        Keyword arguments:
        command -- the command to run
        lock    -- lock object for write on stdout
        """

        stdout = b''
        stderr = b''

        session = self.__run_command(command)

        while not session:
            self.reconnect()
            session = self.__run_command(command)

        while True:
            buf = b''

            # wait for data to be transmitted. if the timeout is hit,
            # ask the user on how to procceed
            if select.select([session], [], [], self.timeout) == ([], [], []):
                assert session

                # writing on stdout needs locking as all run threads could
                # write at the same time to stdout
                if lock:
                    lock.acquire()

                try:
                    if input(
                            'command "%s" timed out on %s. wait? (y/N) ' %
                            (command, self.hostname)).lower() in [
                            'y', 'yes']:
                        continue
                    else:
                        # if the user don't want to wait, raise CommandTimeout
                        # and procceed
                        raise CommandTimeout
                finally:
                    # release lock to allow other command threads to write to
                    # stdout
                    if lock:
                        lock.release()

            try:
                # wait for data on the session's stdout/stderr. if debug is enabled,
                # print the received data
                if session.recv_ready():
                    buf = session.recv(1024)
                    stdout += buf
                    for line in buf.decode('utf-8', 'ignore').split('\n'):
                        if line:
                            logger.debug(line)

                if session.recv_stderr_ready():
                    buf = session.recv_stderr(1024)
                    stderr += buf
                    for line in buf.decode('utf-8', 'ignore').split('\n'):
                        if line:
                            logger.debug(line)

                if not buf:
                    break

            except socket.timeout:
                select.select([], [], [], 1)

        # save the exitcode of the last command and return it
        exitcode = session.recv_exit_status()
        self.close_session(session)

        return (stdout.decode(), stderr.decode(), exitcode)

    def __sftp_open(self):
        try:
            sftp = self.client.open_sftp()
        except (AttributeError, paramiko.ChannelException, paramiko.SSHException):
            if 'sftp' in locals():
                if isinstance(sftp, paramiko.sftp_client.SFTPClient):
                    sftp.close()
            return False
        return sftp

    def __sftp_reconnect(self):
        sftp = self.__sftp_open()
        while not sftp:
            self.reconnect()
            sftp = self.__sftp_open()
        return sftp

    def listdir(self, path='.'):
        """get directory listing of the remote host

        Keyword arguments:
        path   -- remote directory path to list

        """

        logger.debug('getting {!s}:{!s}:{!s} listing'.format(self.hostname, self.port, path))
        sftp = self.__sftp_reconnect()

        listdir = sftp.listdir(path)
        sftp.close()
        return listdir

    def open(self, filename, mode='r', bufsize=-1):
        """open remote file
           default mode is reading
           can be used as context manager
        """

        logger.debug('{0} open({1}, {2})'.format(repr(self), filename, mode))
        logger.debug("  -> self.client.open_sftp")
        sftp = self.__sftp_reconnect()
        logger.debug("  -> sftp.open")
        try:
            ofile = sftp.open(filename, mode, bufsize)
        except BaseException:
            logger.debug(format_exc())
            # TODO: recheck if is needed
            if 'sftp' in locals():
                if isinstance(sftp, paramiko.sftp_client.SFTPClient):
                    sftp.close()
            raise
        return ofile

    def readlink(self, path):
        """ Return the target of a symbolic link (shortcut)."""
        logger.debug("read link {}:{}:{}".format(self.hostname, self.port, path))

        sftp = self.__sftp_reconnect()
        link = sftp.readlink(path)
        sftp.close()
        return link

    def is_active(self):

        return self.client._transport and self.client._transport.is_active()

    def close(self):
        """closes SSH channel to host and disconnects
        Keyword arguments: None
        """
        logger.debug(
            'closing connection to {}:{}'.format(
                self.hostname, self.port))
        self.client.close()
