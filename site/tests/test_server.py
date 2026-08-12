import importlib.util
import json
import os
import tempfile
import threading
import unittest
import urllib.error
import urllib.request
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class SiteServerTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp_dir = tempfile.TemporaryDirectory()
        os.environ["MILLO_SITE_DATABASE"] = str(Path(cls.temp_dir.name) / "feedback.sqlite3")
        os.environ["MILLO_SITE_PUBLIC"] = str(ROOT / "public")
        os.environ.pop("MILLO_MODERATION_URL", None)
        os.environ.pop("MILLO_MODERATION_MODEL", None)
        spec = importlib.util.spec_from_file_location("millo_site", ROOT / "server.py")
        cls.site = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.site)
        cls.site.initialize_database()
        cls.server = cls.site.ThreadingHTTPServer(("127.0.0.1", 0), cls.site.MilloSiteHandler)
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()
        cls.base_url = f"http://127.0.0.1:{cls.server.server_port}"

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()
        cls.thread.join(timeout=2)
        cls.temp_dir.cleanup()

    def request(self, path, *, payload=None):
        data = None if payload is None else json.dumps(payload).encode()
        request = urllib.request.Request(
            self.base_url + path,
            data=data,
            headers={"Content-Type": "application/json", "X-Forwarded-For": "203.0.113.5"},
        )
        with urllib.request.urlopen(request, timeout=2) as response:
            return response.status, json.loads(response.read())

    def test_health_and_security_headers(self):
        with urllib.request.urlopen(self.base_url + "/api/health", timeout=2) as response:
            payload = json.loads(response.read())
            self.assertEqual(payload["status"], "ok")
            self.assertEqual(response.headers["X-Frame-Options"], "DENY")

    def test_feedback_fails_closed_when_llm_is_unavailable(self):
        status, payload = self.request(
            "/api/comments",
            payload={"name": "Тестер", "kind": "idea", "body": "Добавьте настройку скорости preview."},
        )
        self.assertEqual(status, 202)
        self.assertEqual(payload["status"], "pending")
        _, comments = self.request("/api/comments")
        self.assertEqual(comments["items"], [])

    def test_rejects_invalid_category(self):
        with self.assertRaises(urllib.error.HTTPError) as context:
            self.request(
                "/api/comments",
                payload={"name": "Тестер", "kind": "unknown", "body": "Достаточно длинное сообщение"},
            )
        self.assertEqual(context.exception.code, 400)

    def test_static_path_cannot_escape_public_directory(self):
        with self.assertRaises(urllib.error.HTTPError) as context:
            urllib.request.urlopen(self.base_url + "/../server.py", timeout=2)
        self.assertEqual(context.exception.code, 404)


if __name__ == "__main__":
    unittest.main()
