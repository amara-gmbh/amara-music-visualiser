"""Focused loopback bridge checks with a synthetic helper; no real audio."""

import http.client
import importlib.util
import json
import os
import socket
import tempfile
import threading
import time
import unittest
from pathlib import Path

SERVER = Path(__file__).resolve().parents[1] / 'mac-audio-server.py'
spec = importlib.util.spec_from_file_location('mac_audio_server', SERVER)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class BridgeTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        helper = Path(self.temp.name) / 'fake-helper'
        helper.write_text('#!/usr/bin/env python3\nimport os,time\nwhile True:\n os.write(1, bytes(8192))\n time.sleep(.02)\n')
        helper.chmod(0o700)
        self.server = module.AudioServer(0, helper)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.host = self.server.authority
        self.origin = 'http://' + self.host

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=3)
        self.temp.cleanup()

    def request(self, method, path, headers=None):
        client = http.client.HTTPConnection('127.0.0.1', self.server.server_port, timeout=3)
        client.request(method, path, body='' if method == 'POST' else None,
                       headers=headers or {})
        return client, client.getresponse()

    def test_private_origin_and_restricted_assets(self):
        client, response = self.request('POST', '/api/system-audio/status', {'Origin': 'https://evil.example'})
        self.assertEqual(response.status, 403)
        response.read(); client.close()
        client, response = self.request('POST', '/api/system-audio/status')
        self.assertEqual(response.status, 403)
        response.read(); client.close()
        client, response = self.request('POST', '/api/system-audio/status', {'Host': 'evil.example', 'Origin': self.origin})
        self.assertEqual(response.status, 403)
        response.read(); client.close()
        client, response = self.request('GET', '/web/')
        self.assertEqual(response.status, 200)
        self.assertEqual(response.getheader('X-Frame-Options'), 'DENY')
        self.assertIn("frame-ancestors 'none'", response.getheader('Content-Security-Policy'))
        response.read(); client.close()
        client, response = self.request('GET', '/src/params.wgsl')
        self.assertEqual(response.status, 200)
        response.read(); client.close()
        for path in ['/work/', '/Cargo.toml', '/web/mac-audio-server.py', '/web/../Cargo.toml']:
            client, response = self.request('GET', path)
            self.assertEqual(response.status, 404, path)
            response.read(); client.close()

    def test_stream_is_opt_in_single_listener_and_stops(self):
        headers = {'Origin': self.origin}
        client, response = self.request('POST', '/api/system-audio/status', headers)
        self.assertFalse(json.loads(response.read())['active']); client.close()
        stream_client, stream = self.request('POST', '/api/system-audio/stream', headers)
        self.assertEqual(stream.status, 200)
        self.assertEqual(stream.getheader('Content-Type'), 'application/octet-stream')
        self.assertEqual(stream.read(1024), bytes(1024))
        client, response = self.request('POST', '/api/system-audio/status', headers)
        self.assertTrue(json.loads(response.read())['active']); client.close()
        client, response = self.request('POST', '/api/system-audio/stream', headers)
        self.assertEqual(response.status, 409)
        response.read(); client.close()
        client, response = self.request('POST', '/api/system-audio/stop', headers)
        self.assertEqual(response.status, 200)
        response.read(); client.close()
        stream_client.close()
        deadline = time.monotonic() + 3
        while self.server.capture is not None and time.monotonic() < deadline:
            time.sleep(.02)
        self.assertIsNone(self.server.capture)

    def test_absent_helper_fails_without_audio(self):
        self.server.helper = Path(self.temp.name) / 'missing'
        client, response = self.request('POST', '/api/system-audio/stream', {'Origin': self.origin})
        self.assertEqual(response.status, 503)
        self.assertIn('helper', json.loads(response.read())['error'])
        client.close()

    def test_disconnecting_client_stops_helper(self):
        client, response = self.request('POST', '/api/system-audio/stream', {'Origin': self.origin})
        self.assertEqual(response.status, 200)
        self.assertEqual(response.read(1024), bytes(1024))
        response.close()
        client.close()
        deadline = time.monotonic() + 3
        while self.server.capture is not None and time.monotonic() < deadline:
            time.sleep(.02)
        self.assertIsNone(self.server.capture)

    def test_start_timeout_returns_only_one_response(self):
        sleeper = Path(self.temp.name) / 'silent-helper'
        sleeper.write_text('#!/usr/bin/env python3\nimport time\ntime.sleep(5)\n')
        sleeper.chmod(0o700)
        self.server.helper = sleeper
        previous = module.START_TIMEOUT
        module.START_TIMEOUT = .1
        try:
            with socket.create_connection(('127.0.0.1', self.server.server_port), timeout=3) as sock:
                request = (f'POST /api/system-audio/stream HTTP/1.1\r\nHost: {self.host}\r\n'
                           f'Origin: {self.origin}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n')
                sock.sendall(request.encode())
                wire = b''
                while True:
                    block = sock.recv(4096)
                    if not block:
                        break
                    wire += block
            header, body = wire.split(b'\r\n\r\n', 1)
            self.assertIn(b' 504 ', header)
            self.assertEqual(wire.count(b'HTTP/1.1 '), 1)
            length = next(int(line.split(b':', 1)[1]) for line in header.split(b'\r\n')
                          if line.lower().startswith(b'content-length:'))
            self.assertEqual(len(body), length)
            self.assertIn('did not start', json.loads(body)['error'])
        finally:
            module.START_TIMEOUT = previous


if __name__ == '__main__':
    unittest.main()
