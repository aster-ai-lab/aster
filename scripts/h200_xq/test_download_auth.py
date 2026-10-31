import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import download_auth


class DownloadCredentialTests(unittest.TestCase):
    def test_private_credentials_are_read_without_putting_the_secret_in_the_environment(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(download_auth.Path, 'home', return_value=Path(directory)), \
                patch.dict(download_auth.os.environ, {'TEST_SCOPE': 'active'}, clear=True):
            root = Path(directory)/'.aster-h200-xq-download'
            root.mkdir(mode=0o700)
            (root/'auth.json').write_text(json.dumps({'username': 'aster-download', 'password': 'a'*64}))
            (root/'.curlrc').write_text('proxy-user = "test-only"\n')
            for path in root.iterdir():path.chmod(0o600)
            self.assertEqual(download_auth.socks_authentication(), (b'aster-download', b'a'*64))
            environment = download_auth.curl_environment('socks5h://127.0.0.1:18095')
            self.assertEqual(environment, {'TEST_SCOPE': 'active', 'CURL_HOME': str(root)})
            self.assertEqual(download_auth.curl_environment('socks5h://other.example:1080'), {'TEST_SCOPE': 'active'})

    def test_readable_credential_files_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(download_auth.Path, 'home', return_value=Path(directory)):
            root = Path(directory)/'.aster-h200-xq-download'
            root.mkdir(mode=0o700)
            (root/'auth.json').write_text('{}')
            (root/'auth.json').chmod(0o644)
            with self.assertRaisesRegex(RuntimeError, 'permissions'):
                download_auth.socks_authentication()


if __name__ == '__main__':
    unittest.main()
