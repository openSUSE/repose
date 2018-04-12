import sys
import threading
import time
from queue import Queue


queue = Queue()


class ThreadedMethod(threading.Thread):

    def __init__(self, queue):
        threading.Thread.__init__(self)
        self.queue = queue

    def run(self):
        while True:
            try:
                (method, parameter) = self.queue.get(timeout=10)
            except BaseException:
                return

            try:
                method(*parameter)
            except BaseException:
                raise
            finally:
                try:
                    self.queue.task_done()
                except ValueError:
                    pass  # already removed by ctrl+c


class RunCommand(object):

    def __init__(self, targets, command):
        self.targets = targets
        self.command = command

    def run(self):
        lock = threading.Lock()

        try:
            for target in self.targets:
                thread = ThreadedMethod(queue)
                thread.daemon = True
                thread.start()
                if isinstance(self.command, dict):
                    queue.put([self.targets[target].run, [self.command[target], lock]])
                elif isinstance(self.command, str):
                    queue.put([self.targets[target].run, [self.command, lock]])

            while queue.unfinished_tasks:
                spinner(lock)

            queue.join()

        except KeyboardInterrupt:
            print('stopping command queue, please wait.')
            try:
                while queue.unfinished_tasks:
                    spinner(lock)
            except KeyboardInterrupt:
                for target in self.targets:
                    try:
                        self.targets[target].connection.close_session()
                    except Exception:
                        pass
                try:
                    thread.queue.task_done()
                except ValueError:
                    pass

            queue.join()
            raise


def spinner(lock=None):
    """simple spinner to show some process"""

    for pos in ['|', '/', '-', '\\']:
        if lock is not None:
            lock.acquire()

        try:
            sys.stdout.write('processing... [{!s}]\r'.format(pos))
            sys.stdout.flush()
        finally:
            if lock is not None:
                lock.release()

        time.sleep(0.3)
