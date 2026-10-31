import json
from pathlib import Path
import socket
import subprocess
import shutil
import unittest


@unittest.skipUnless(shutil.which('node'), 'download exit checks require Node')
class AuthProxyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        with socket.socket() as listener:
            listener.bind(('127.0.0.1', 0))
            cls.port = listener.getsockname()[1]
        cls.password = b'x'*64
        cls.process = subprocess.Popen(['node', str(Path(__file__).with_name('download_exit.cjs'))],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
            creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
        cls.process.stdin.write(json.dumps({'username': 'aster-download', 'password': cls.password.decode(), 'port': cls.port}).encode())
        cls.process.stdin.close()
        if not json.loads(cls.process.stdout.readline())['ready']:
            raise RuntimeError('test proxy did not start')

    @classmethod
    def tearDownClass(cls):
        cls.process.terminate()
        cls.process.wait(timeout=10)
        cls.process.stdout.close()

    def receive(self, connection, count):
        result = b''
        while len(result) < count:
            block = connection.recv(count-len(result))
            if not block:
                raise RuntimeError('truncated test response')
            result += block
        return result

    def connect(self, password):
        connection = socket.create_connection(('127.0.0.1', self.port), timeout=5)
        connection.sendall(b'\x05\x01\x02')
        self.assertEqual(self.receive(connection, 2), b'\x05\x02')
        user = b'aster-download'
        for byte in b'\x01'+bytes([len(user)])+user+bytes([len(password)])+password:
            connection.sendall(bytes([byte]))
        return connection

    def test_missing_authentication_is_rejected(self):
        with socket.create_connection(('127.0.0.1', self.port), timeout=5) as connection:
            connection.sendall(b'\x05\x01\x00')
            self.assertEqual(self.receive(connection, 2), b'\x05\xff')

    def test_invalid_password_is_rejected(self):
        with self.connect(b'wrong') as connection:
            self.assertEqual(self.receive(connection, 2), b'\x01\x01')

    def test_authenticated_client_cannot_reach_an_unlisted_host(self):
        with self.connect(self.password) as connection:
            self.assertEqual(self.receive(connection, 2), b'\x01\x00')
            host = b'localhost'
            connection.sendall(b'\x05\x01\x00\x03'+bytes([len(host)])+host+b'\x01\xbb')
            self.assertEqual(self.receive(connection, 10)[:2], b'\x05\x02')


if __name__ == '__main__':
    unittest.main(verbosity=2)
