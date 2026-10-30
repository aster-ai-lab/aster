"""Run the actual CI object-fixture startup script without network access."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

from release_governance import workflow_script


class ObjectFixtureStartup(unittest.TestCase):
    def probe(self, health_status=0, bucket_status=0):
        script = workflow_script(
            "backend.yml", "Start the campaign object store and create its bucket"
        )
        with tempfile.TemporaryDirectory(prefix="aster-object-fixture-") as directory:
            calls = Path(directory) / "calls"
            env = dict(os.environ, CALLS=str(calls), HEALTH_STATUS=str(health_status),
                       BUCKET_STATUS=str(bucket_status),
                       A3S_CLOUD_TEST_S3_ENDPOINT="http://127.0.0.1:9000",
                       A3S_CLOUD_TEST_S3_BUCKET="fixture-bucket",
                       A3S_CLOUD_TEST_S3_ACCESS_KEY_ID="fixture-key",
                       A3S_CLOUD_TEST_S3_SECRET_ACCESS_KEY="fixture-secret")
            commands = '''set -euo pipefail
docker() {
  printf 'docker %s\n' "$*" >> "$CALLS"
  if [[ "$*" == *' s3 mb '* ]]; then return "$BUCKET_STATUS"; fi
}
curl() { printf 'health\n' >> "$CALLS"; return "$HEALTH_STATUS"; }
sleep() { printf 'sleep\n' >> "$CALLS"; }
'''
            result = subprocess.run(["bash", "-c", commands + script], env=env,
                                    capture_output=True, text=True, timeout=10)
            return result, calls.read_text().splitlines()

    def test_bucket_creation_follows_health_and_uses_pinned_images(self):
        result, calls = self.probe()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(len(calls), 3)
        self.assertRegex(calls[0], r"rustfs/rustfs:1\.0\.0@sha256:[0-9a-f]{64} /data$")
        self.assertEqual(calls[1], "health")
        self.assertRegex(calls[2], r"amazon/aws-cli:2\.31\.18@sha256:[0-9a-f]{64} ")
        self.assertTrue(calls[2].endswith("s3 mb s3://fixture-bucket"))

    def test_unhealthy_store_refuses_bucket_creation_at_the_existing_limit(self):
        result, calls = self.probe(health_status=1)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(calls.count("health"), 60)
        self.assertEqual(calls.count("sleep"), 59)
        self.assertEqual(calls[-1], "docker logs a3s-s3")
        self.assertFalse(any(" s3 mb " in call for call in calls))

    def test_bucket_failure_propagates(self):
        result, calls = self.probe(bucket_status=22)
        self.assertEqual(result.returncode, 22)
        self.assertEqual(len(calls), 3)


if __name__ == "__main__":
    unittest.main()
